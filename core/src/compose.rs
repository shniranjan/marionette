use std::path::PathBuf;
use std::process::Command;

/// Run docker compose commands against a stack directory.
pub struct ComposeRunner {
    stacks_dir: PathBuf,
}

impl ComposeRunner {
    pub fn new(stacks_dir: PathBuf) -> Self {
        Self { stacks_dir }
    }

    /// Run `docker compose` with the given args in a stack subdirectory.
    pub fn run(&self, stack_name: &str, args: &[&str]) -> Result<String, String> {
        let dir = self.stacks_dir.join(stack_name);
        if !dir.exists() {
            return Err(format!("Stack directory not found: {}", dir.display()));
        }

        let output = Command::new("docker")
            .args(["compose"])
            .args(args)
            .current_dir(&dir)
            .output()
            .map_err(|e| format!("Failed to run docker compose: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if output.status.success() {
            Ok(stdout)
        } else {
            Err(format!(
                "docker compose failed: {}",
                if stderr.is_empty() { stdout } else { stderr }
            ))
        }
    }

    /// Check if a stack has running services by running `docker compose ps`.
    /// Returns "running" if at least one service is up, "stopped" otherwise.
    pub fn stack_status(&self, stack_name: &str) -> String {
        match self.run(stack_name, &["ps", "--format", "{{.Status}}"]) {
            Ok(output) => {
                if output.lines().any(|l| l.starts_with("Up") || l.starts_with("running")) {
                    "running".to_string()
                } else if output.trim().is_empty() {
                    "stopped".to_string()
                } else {
                    "stopped".to_string()
                }
            }
            Err(_) => "unknown".to_string(),
        }
    }

    /// List all stacks by scanning the stacks directory for compose files.
    /// Accepts both `docker-compose.yml` and `compose.yml`.
    pub fn list_stacks(&self) -> Vec<crate::models::StackSummary> {
        let mut stacks = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&self.stacks_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let compose_file = path.join("docker-compose.yml");
                    let alt_file = path.join("compose.yml");
                    let actual_file = if compose_file.exists() {
                        &compose_file
                    } else if alt_file.exists() {
                        &alt_file
                    } else {
                        continue;
                    };

                    let name = path.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();

                    // Count services by parsing the yml (simple grep for 'image:')
                    let services = std::fs::read_to_string(actual_file)
                        .map(|c| c.lines().filter(|l| l.trim().starts_with("image:")).count())
                        .unwrap_or(0);

                    let status = self.stack_status(&name);

                    stacks.push(crate::models::StackSummary {
                        name,
                        services,
                        status,
                        file: actual_file.to_string_lossy().to_string(),
                    });
                }
            }
        }
        stacks
    }
}

/// Read a compose file from a Docker endpoint via bollard.
///
/// **Local endpoint** (`unix://`): reads directly from the filesystem with `std::fs::read_to_string`.
/// This avoids the unnecessary alpine container overhead and the bind-mount path bug
/// (where the container-internal path like `/stacks` is not a valid host path for the bind).
///
/// **Remote endpoint**: creates an ephemeral alpine container with the stacks directory
/// bind-mounted, runs `cat` to read the file, and captures stdout from container logs.
/// Uses `host_stacks_dir` (the real host path) for the bind mount source.
pub async fn read_compose_remote(
    docker: &bollard::Docker,
    stacks_dir: &str,
    host_stacks_dir: Option<&str>,
    stack_name: &str,
    is_local: bool,
) -> Result<String, String> {
    // Fast path: local endpoint — read directly from filesystem
    if is_local {
        let base = stacks_dir.trim_end_matches('/');
        for fname in &["docker-compose.yml", "compose.yml"] {
            let path = format!("{}/{}/{}", base, stack_name, fname);
            if let Ok(content) = std::fs::read_to_string(&path) {
                return Ok(content);
            }
        }
        return Err(format!(
            "Compose file not found for stack '{}' in {} — checked docker-compose.yml and compose.yml",
            stack_name, stacks_dir
        ));
    }

    // Remote endpoint: create alpine container with bind mount
    use bollard::container::{
        Config, CreateContainerOptions, LogOutput, LogsOptions, RemoveContainerOptions,
        StartContainerOptions,
    };
    use bollard::models::HostConfig;
    use futures::StreamExt;

    let bind_source = host_stacks_dir.unwrap_or(stacks_dir);

    // Try docker-compose.yml first, then compose.yml
    let file_paths = [
        format!("{}/{}/docker-compose.yml", stacks_dir.trim_end_matches('/'), stack_name),
        format!("{}/{}/compose.yml", stacks_dir.trim_end_matches('/'), stack_name),
    ];

    // Find which compose file exists
    let mut found_content: Option<String> = None;

    for file_path in &file_paths {
        let container_name = format!("mari-read-{}", stack_name.chars().take(20).collect::<String>());

        // Create alpine container with stacks dir mounted, command: cat <file>
        let container = match docker
            .create_container(
                Some(CreateContainerOptions {
                    name: container_name.clone(),
                    platform: None,
                }),
                Config {
                    image: Some("alpine:latest"),
                    cmd: Some(vec!["cat", file_path.as_str()]),
                    host_config: Some(HostConfig {
                        binds: Some(vec![format!("{}:{}:ro", bind_source, stacks_dir)]),
                        auto_remove: Some(true),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            )
            .await
        {
            Ok(c) => c,
            Err(_) => continue,
        };

        // Start the container
        if docker
            .start_container(&container.id, None::<StartContainerOptions<String>>)
            .await
            .is_err()
        {
            let _ = docker
                .remove_container(&container.id, None::<RemoveContainerOptions>)
                .await;
            continue;
        }

        // Collect logs (stdout)
        let mut logs = docker.logs(
            &container.id,
            Some(LogsOptions::<String> {
                follow: true,
                stdout: true,
                stderr: true,
                ..Default::default()
            }),
        );

        let mut content = String::new();
        while let Some(chunk) = logs.next().await {
            match chunk {
                Ok(LogOutput::StdOut { message }) => {
                    content.push_str(&String::from_utf8_lossy(&message));
                }
                Ok(LogOutput::StdErr { message }) => {
                    let err = String::from_utf8_lossy(&message);
                    if err.contains("No such file") || err.contains("cat:") {
                        // File not found on this attempt, try next
                        break;
                    }
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }

        // Cleanup
        let _ = docker
            .remove_container(&container.id, None::<RemoveContainerOptions>)
            .await;

        if !content.trim().is_empty() {
            found_content = Some(content);
            break;
        }
    }

    found_content.ok_or_else(|| {
        format!(
            "Compose file not found for stack '{}' in {} — checked docker-compose.yml and compose.yml",
            stack_name, stacks_dir
        )
    })
}
