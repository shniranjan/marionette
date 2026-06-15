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
            Err(format!("docker compose failed: {}", if stderr.is_empty() { stdout } else { stderr }))
        }
    }

    /// List all stacks by scanning the stacks directory for docker-compose files.
    pub fn list_stacks(&self) -> Vec<crate::models::StackSummary> {
        let mut stacks = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&self.stacks_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let compose_file = path.join("docker-compose.yml");
                    if compose_file.exists() {
                        let name = path.file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default();

                        // Count services by parsing the yml (simple grep for 'image:')
                        let services = std::fs::read_to_string(&compose_file)
                            .map(|c| c.lines().filter(|l| l.trim().starts_with("image:")).count())
                            .unwrap_or(0);

                        stacks.push(crate::models::StackSummary {
                            name,
                            services,
                            status: "unknown".to_string(),
                            file: compose_file.to_string_lossy().to_string(),
                        });
                    }
                }
            }
        }
        stacks
    }
}
