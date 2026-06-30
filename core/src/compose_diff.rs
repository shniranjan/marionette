// ── Compose Template Diff Engine ─────────────────────────────────
// Parses two Docker Compose YAMLs and produces a structured diff
// suitable for migration planning. Handles volume changes, service
// changes, image updates, env var diffs, port mappings, architecture
// warnings, and database service detection.

use serde_yaml::Value;

/// Structured diff between two compose files.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposeDiff {
    pub volume_changes: Vec<VolumeChange>,
    pub service_changes: Vec<ServiceChange>,
    pub image_changes: Vec<ImageChange>,
    pub env_changes: Vec<EnvChange>,
    pub port_changes: Vec<PortChange>,
    pub architecture: Option<ArchitectureInfo>,
    pub database_services: Vec<DatabaseService>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeChange {
    pub name: String,
    pub change_type: String, // "added", "removed", "renamed", "modified"
    pub source_name: Option<String>,
    pub driver: Option<String>,
    pub details: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceChange {
    pub name: String,
    pub change_type: String, // "added", "removed", "modified", "unchanged"
    pub image_old: Option<String>,
    pub image_new: Option<String>,
    pub details: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageChange {
    pub service_name: String,
    pub old_image: String,
    pub new_image: String,
    pub major_version_change: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvChange {
    pub service_name: String,
    pub var_name: String,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub is_sensitive: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortChange {
    pub service_name: String,
    pub change_type: String, // "added", "removed", "modified"
    pub port_mapping: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchitectureInfo {
    pub source_arch: Option<String>,
    pub target_arch: Option<String>,
    pub mismatch: bool,
    pub warning: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseService {
    pub service_name: String,
    pub db_type: DatabaseType,
    pub image: String,
    pub version: Option<String>,
    pub has_replication: bool,
    pub pre_transfer_commands: Vec<String>,
    pub post_transfer_commands: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum DatabaseType {
    PostgreSQL,
    MySQL,
    MongoDB,
    Redis,
    Other(String),
}

impl std::fmt::Display for DatabaseType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PostgreSQL => write!(f, "PostgreSQL"),
            Self::MySQL => write!(f, "MySQL"),
            Self::MongoDB => write!(f, "MongoDB"),
            Self::Redis => write!(f, "Redis"),
            Self::Other(s) => write!(f, "{}", s),
        }
    }
}

/// Generate a complete diff between two compose YAMLs.
pub fn diff_compose(
    source_yaml: &str,
    target_yaml: &str,
    source_arch: Option<&str>,
    target_arch: Option<&str>,
) -> Result<ComposeDiff, String> {
    let source: Value =
        serde_yaml::from_str(source_yaml).map_err(|e| format!("Failed to parse source compose: {}", e))?;
    let target: Value =
        serde_yaml::from_str(target_yaml).map_err(|e| format!("Failed to parse target compose: {}", e))?;

    let mut diff = ComposeDiff {
        volume_changes: Vec::new(),
        service_changes: Vec::new(),
        image_changes: Vec::new(),
        env_changes: Vec::new(),
        port_changes: Vec::new(),
        architecture: None,
        database_services: Vec::new(),
        warnings: Vec::new(),
    };

    // ── Architecture check ─────────────────────────────────
    if let (Some(src), Some(tgt)) = (source_arch, target_arch) {
        let mismatch = src != tgt;
        diff.architecture = Some(ArchitectureInfo {
            source_arch: Some(src.to_string()),
            target_arch: Some(tgt.to_string()),
            mismatch,
            warning: if mismatch {
                Some(format!(
                    "Architecture mismatch: source is {}, target is {}. Images must be multi-arch or re-tagged.",
                    src, tgt
                ))
            } else {
                None
            },
        });
        if mismatch {
            diff.warnings.push(format!(
                "Architecture mismatch: {} → {}",
                src, tgt
            ));
        }
    }

    // ── Volume diff ───────────────────────────────────────
    let src_volumes = source.get("volumes").and_then(|v| v.as_mapping());
    let tgt_volumes = target.get("volumes").and_then(|v| v.as_mapping());

    // Volumes in source but not target → "removed"
    if let Some(vols) = src_volumes {
        for (name, _) in vols {
            let name_str = name.as_str().unwrap_or("??");
            if tgt_volumes.map_or(true, |tv| !tv.contains_key(name)) {
                diff.volume_changes.push(VolumeChange {
                    name: name_str.to_string(),
                    change_type: "removed".to_string(),
                    source_name: Some(name_str.to_string()),
                    driver: None,
                    details: Some("Volume exists in source but not target compose".to_string()),
                });
            }
        }
    }

    // Volumes in target but not source → "added"
    if let Some(vols) = tgt_volumes {
        for (name, _) in vols {
            let name_str = name.as_str().unwrap_or("??");
            if src_volumes.map_or(true, |sv| !sv.contains_key(name)) {
                diff.volume_changes.push(VolumeChange {
                    name: name_str.to_string(),
                    change_type: "added".to_string(),
                    source_name: None,
                    driver: None,
                    details: Some("Volume defined in target but not in source".to_string()),
                });
            }
        }
    }

    // Detect renamed volumes (heuristic: different names but same driver)
    if let (Some(sv), Some(tv)) = (src_volumes, tgt_volumes) {
        // Collect source drivers
        let mut src_drivers: Vec<(String, String)> = Vec::new();
        for (name, details) in sv {
            if let Some(driver) = details.get("driver").and_then(|d| d.as_str()) {
                src_drivers.push((name.as_str().unwrap_or("").to_string(), driver.to_string()));
            }
        }
        for (name, details) in tv {
            if let Some(driver) = details.get("driver").and_then(|d| d.as_str()) {
                let name_str = name.as_str().unwrap_or("").to_string();
                // Check if a source volume with same driver but different name exists
                for (src_name, src_driver) in &src_drivers {
                    if src_driver == driver && src_name != &name_str && sv.contains_key(&Value::String(src_name.clone())) {
                        // Possible rename
                        diff.volume_changes.push(VolumeChange {
                            name: name_str.clone(),
                            change_type: "renamed".to_string(),
                            source_name: Some(src_name.clone()),
                            driver: Some(driver.to_string()),
                            details: Some(format!("Possible rename: '{}' → '{}' (same driver: {})", src_name, name_str, driver)),
                        });
                    }
                }
            }
        }
    }

    // ── Service diff ──────────────────────────────────────
    let src_services = source.get("services").and_then(|s| s.as_mapping());
    let tgt_services = target.get("services").and_then(|s| s.as_mapping());

    // Services in source
    if let Some(svcs) = src_services {
        for (name, details) in svcs {
            let name_str = name.as_str().unwrap_or("??");
            let src_image = details.get("image").and_then(|i| i.as_str());

            if let Some(tgt_details) = tgt_services.and_then(|t| t.get(name)) {
                let tgt_image = tgt_details.get("image").and_then(|i| i.as_str());

                if src_image != tgt_image {
                    let major_change = src_image.map_or(false, |si| {
                        tgt_image.map_or(false, |ti| {
                            let sv = img_major_version(si);
                            let tv = img_major_version(ti);
                            sv != tv
                        })
                    });

                    diff.service_changes.push(ServiceChange {
                        name: name_str.to_string(),
                        change_type: "modified".to_string(),
                        image_old: src_image.map(|s| s.to_string()),
                        image_new: tgt_image.map(|s| s.to_string()),
                        details: Some("Image changed".to_string()),
                    });

                    if let (Some(old), Some(new)) = (src_image, tgt_image) {
                        diff.image_changes.push(ImageChange {
                            service_name: name_str.to_string(),
                            old_image: old.to_string(),
                            new_image: new.to_string(),
                            major_version_change: major_change,
                        });
                    }
                } else {
                    diff.service_changes.push(ServiceChange {
                        name: name_str.to_string(),
                        change_type: "unchanged".to_string(),
                        image_old: src_image.map(|s| s.to_string()),
                        image_new: tgt_image.map(|s| s.to_string()),
                        details: None,
                    });
                }

                // ── Env var diff ──────────────────────────
                let src_env = details.get("environment");
                let tgt_env = tgt_details.get("environment");
                diff_env_vars(name_str, src_env, tgt_env, &mut diff.env_changes);

                // ── Port diff ─────────────────────────────
                let src_ports = details.get("ports");
                let tgt_ports = tgt_details.get("ports");
                diff_ports(name_str, src_ports, tgt_ports, &mut diff.port_changes);

                // ── Database detection ────────────────────
                if let Some(image) = src_image {
                    if let Some(db_type) = detect_database_type(image) {
                        diff.database_services.push(DatabaseService {
                            service_name: name_str.to_string(),
                            db_type: db_type.clone(),
                            image: image.to_string(),
                            version: img_version(image),
                            has_replication: false,
                            pre_transfer_commands: generate_db_pre_commands(&db_type, name_str),
                            post_transfer_commands: generate_db_post_commands(&db_type, name_str),
                        });
                    }
                }
            } else {
                // Service removed in target
                diff.service_changes.push(ServiceChange {
                    name: name_str.to_string(),
                    change_type: "removed".to_string(),
                    image_old: src_image.map(|s| s.to_string()),
                    image_new: None,
                    details: Some("Service exists in source but not target".to_string()),
                });
            }
        }
    }

    // Services in target but not source → "added"
    if let Some(svcs) = tgt_services {
        for (name, details) in svcs {
            let name_str = name.as_str().unwrap_or("??");
            if src_services.map_or(true, |s| !s.contains_key(name)) {
                let image = details.get("image").and_then(|i| i.as_str());
                diff.service_changes.push(ServiceChange {
                    name: name_str.to_string(),
                    change_type: "added".to_string(),
                    image_old: None,
                    image_new: image.map(|s| s.to_string()),
                    details: Some("Service added in target".to_string()),
                });
            }
        }
    }

    Ok(diff)
}

// ── Helpers ──────────────────────────────────────────────────────

fn diff_env_vars(
    service_name: &str,
    src_env: Option<&Value>,
    tgt_env: Option<&Value>,
    changes: &mut Vec<EnvChange>,
) {
    let src_map = env_to_map(src_env);
    let tgt_map = env_to_map(tgt_env);

    // Check vars in source
    for (key, val) in &src_map {
        let is_sensitive = is_sensitive_env(key);
        match tgt_map.get(key) {
            Some(tgt_val) if tgt_val != val => {
                changes.push(EnvChange {
                    service_name: service_name.to_string(),
                    var_name: key.clone(),
                    old_value: if is_sensitive {
                        Some("••••".to_string())
                    } else {
                        Some(val.clone())
                    },
                    new_value: if is_sensitive {
                        Some("••••".to_string())
                    } else {
                        Some(tgt_val.clone())
                    },
                    is_sensitive,
                });
            }
            None => {
                changes.push(EnvChange {
                    service_name: service_name.to_string(),
                    var_name: key.clone(),
                    old_value: if is_sensitive {
                        Some("••••".to_string())
                    } else {
                        Some(val.clone())
                    },
                    new_value: None,
                    is_sensitive,
                });
            }
            _ => {} // unchanged
        }
    }

    // Vars in target but not source
    for (key, val) in &tgt_map {
        if !src_map.contains_key(key) {
            let is_sensitive = is_sensitive_env(key);
            changes.push(EnvChange {
                service_name: service_name.to_string(),
                var_name: key.clone(),
                old_value: None,
                new_value: if is_sensitive {
                    Some("••••".to_string())
                } else {
                    Some(val.clone())
                },
                is_sensitive,
            });
        }
    }
}

/// Convert compose environment (list or map) to HashMap.
fn env_to_map(env: Option<&Value>) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    match env {
        Some(Value::Mapping(m)) => {
            for (k, v) in m {
                if let (Some(key), Some(val)) = (k.as_str(), v.as_str()) {
                    map.insert(key.to_string(), val.to_string());
                }
            }
        }
        Some(Value::Sequence(seq)) => {
            for item in seq {
                if let Some(line) = item.as_str() {
                    if let Some((k, v)) = line.split_once('=') {
                        map.insert(k.to_string(), v.to_string());
                    }
                }
            }
        }
        _ => {}
    }
    map
}

fn diff_ports(
    service_name: &str,
    src_ports: Option<&Value>,
    tgt_ports: Option<&Value>,
    changes: &mut Vec<PortChange>,
) {
    let src_set = ports_to_set(src_ports);
    let tgt_set = ports_to_set(tgt_ports);

    for p in &src_set {
        if !tgt_set.contains(p) {
            changes.push(PortChange {
                service_name: service_name.to_string(),
                change_type: "removed".to_string(),
                port_mapping: p.clone(),
            });
        }
    }
    for p in &tgt_set {
        if !src_set.contains(p) {
            changes.push(PortChange {
                service_name: service_name.to_string(),
                change_type: "added".to_string(),
                port_mapping: p.clone(),
            });
        }
    }
}

fn ports_to_set(ports: Option<&Value>) -> std::collections::HashSet<String> {
    let mut set = std::collections::HashSet::new();
    match ports {
        Some(Value::Sequence(seq)) => {
            for item in seq {
                let s = match item {
                    Value::String(s) => s.clone(),
                    Value::Number(n) => n.to_string(),
                    _ => continue,
                };
                set.insert(s);
            }
        }
        _ => {}
    }
    set
}

fn is_sensitive_env(key: &str) -> bool {
    let lower = key.to_lowercase();
    lower.contains("password")
        || lower.contains("secret")
        || lower.contains("key")
        || lower.contains("token")
        || lower.contains("credential")
        || lower.contains("auth")
}

// ── Database Detection ──────────────────────────────────────────

/// Detect database type from a Docker image name.
pub fn detect_database_type(image: &str) -> Option<DatabaseType> {
    let img = image.to_lowercase();
    if img.contains("postgres") || img.contains("timescaledb") || img.contains("postgis") {
        Some(DatabaseType::PostgreSQL)
    } else if img.contains("mysql") || img.contains("mariadb") || img.contains("percona") {
        Some(DatabaseType::MySQL)
    } else if img.contains("mongo") {
        Some(DatabaseType::MongoDB)
    } else if img.contains("redis") {
        Some(DatabaseType::Redis)
    } else {
        None
    }
}

/// Extract image version from an image tag string.
pub fn img_version(image: &str) -> Option<String> {
    image.split(':').nth(1).map(|s| s.to_string())
}

/// Extract major version number for comparison.
pub fn img_major_version(image: &str) -> Option<u32> {
    img_version(image).and_then(|v| {
        v.split('.').next().and_then(|s| s.parse::<u32>().ok())
    })
}

/// Generate pre-transfer database commands (run on source).
pub fn generate_db_pre_commands(db_type: &DatabaseType, service_name: &str) -> Vec<String> {
    match db_type {
        DatabaseType::PostgreSQL => vec![
            format!("# For service '{}': set database to read-only before transfer", service_name),
            "# docker exec <container> psql -U postgres -c \"ALTER DATABASE dbname SET default_transaction_read_only = on;\"".to_string(),
        ],
        DatabaseType::MySQL => vec![
            format!("# For service '{}': flush and lock before transfer", service_name),
            "# docker exec <container> mysql -u root -e \"FLUSH TABLES WITH READ LOCK;\"".to_string(),
        ],
        DatabaseType::MongoDB => vec![
            format!("# For service '{}': fsync and lock before transfer", service_name),
            "# docker exec <container> mongosh --eval \"db.fsyncLock()\"".to_string(),
        ],
        DatabaseType::Redis => vec![
            format!("# For service '{}': trigger BGSAVE before transfer", service_name),
            "# docker exec <container> redis-cli BGSAVE".to_string(),
        ],
        DatabaseType::Other(_) => vec![
            format!("# For service '{}': review database-specific pre-transfer steps", service_name),
        ],
    }
}

/// Generate post-transfer database commands (run on target).
pub fn generate_db_post_commands(db_type: &DatabaseType, service_name: &str) -> Vec<String> {
    match db_type {
        DatabaseType::PostgreSQL => vec![
            format!("# For service '{}': verify and enable writes on target", service_name),
            "# docker exec <container> psql -U postgres -c \"ALTER DATABASE dbname SET default_transaction_read_only = off;\"".to_string(),
        ],
        DatabaseType::MySQL => vec![
            format!("# For service '{}': verify data on target", service_name),
            "# docker exec <container> mysql -u root -e \"SHOW DATABASES;\"".to_string(),
        ],
        DatabaseType::MongoDB => vec![
            format!("# For service '{}': unlock on target", service_name),
            "# docker exec <container> mongosh --eval \"db.fsyncUnlock()\"".to_string(),
        ],
        DatabaseType::Redis => vec![
            format!("# For service '{}': verify data on target", service_name),
            "# docker exec <container> redis-cli INFO persistence".to_string(),
        ],
        DatabaseType::Other(_) => vec![
            format!("# For service '{}': review database-specific post-transfer steps", service_name),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_postgres() {
        assert_eq!(
            detect_database_type("postgres:15"),
            Some(DatabaseType::PostgreSQL)
        );
        assert_eq!(
            detect_database_type("timescaledb:latest"),
            Some(DatabaseType::PostgreSQL)
        );
    }

    #[test]
    fn test_img_version() {
        assert_eq!(img_version("postgres:15.4"), Some("15.4".to_string()));
        assert_eq!(img_version("alpine:latest"), Some("latest".to_string()));
        assert_eq!(img_version("nginx"), None);
    }

    #[test]
    fn test_img_major_version() {
        assert_eq!(img_major_version("postgres:15.4"), Some(15));
        assert_eq!(img_major_version("mysql:8.0"), Some(8));
        assert_eq!(img_major_version("alpine:latest"), None);
    }

    #[test]
    fn test_diff_simple_service_added() {
        let src = "services:\n  web:\n    image: nginx:1.24\n";
        let tgt = "services:\n  web:\n    image: nginx:1.25\n  db:\n    image: postgres:15\n";
        let diff = diff_compose(src, tgt, None, None).unwrap();
        assert_eq!(diff.service_changes.len(), 2);
        assert!(diff.service_changes.iter().any(|s| s.name == "db" && s.change_type == "added"));
        assert!(diff.service_changes.iter().any(|s| s.name == "web" && s.change_type == "modified"));
    }

    #[test]
    fn test_diff_architecture_mismatch() {
        let src = "services:\n  app:\n    image: alpine\n";
        let diff = diff_compose(src, src, Some("arm64"), Some("amd64")).unwrap();
        assert!(diff.architecture.as_ref().unwrap().mismatch);
        assert!(!diff.warnings.is_empty());
    }

    #[test]
    fn test_diff_database_detection() {
        let src = "services:\n  db:\n    image: postgres:15\n  cache:\n    image: redis:7\n";
        let diff = diff_compose(src, src, None, None).unwrap();
        assert_eq!(diff.database_services.len(), 2);
    }
}
