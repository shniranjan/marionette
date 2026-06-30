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

/// Parsed semantic version from a Docker image tag.
#[derive(Debug, Clone, serde::Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseVersion {
    pub major: u32,
    pub minor: Option<u32>,
    pub patch: Option<u32>,
    pub variant: Option<String>,
}

impl DatabaseVersion {
    /// True if this version is at least the given major.minor.
    pub fn at_least(&self, major: u32, minor: u32) -> bool {
        self.major > major || (self.major == major && self.minor.unwrap_or(0) >= minor)
    }

    /// True if this version is at least the given major version.
    pub fn at_least_major(&self, major: u32) -> bool {
        self.major >= major
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
                        let parsed_version = parse_image_version(image);
                        diff.database_services.push(DatabaseService {
                            service_name: name_str.to_string(),
                            db_type: db_type.clone(),
                            image: image.to_string(),
                            version: img_version(image),
                            has_replication: false,
                            pre_transfer_commands: generate_db_pre_commands(
                                &db_type,
                                name_str,
                                parsed_version.as_ref(),
                            ),
                            post_transfer_commands: generate_db_post_commands(
                                &db_type,
                                name_str,
                                parsed_version.as_ref(),
                            ),
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

/// Parse a Docker image tag into a structured DatabaseVersion.
///
/// Handles formats:
///   - "postgres:15.4-alpine" → major=15, minor=4, variant="alpine"
///   - "postgres:15"          → major=15
///   - "postgres:latest"      → None
///   - "mysql:8.0.36-debian"  → major=8, minor=0, patch=36, variant="debian"
///   - "mariadb:10.11"        → major=10, minor=11
pub fn parse_image_version(image: &str) -> Option<DatabaseVersion> {
    let tag = img_version(image)?;
    let tag_lower = tag.to_lowercase();

    // "latest" or other non-numeric tags
    if tag_lower == "latest" {
        return None;
    }

    // Split version from variant on first '-'
    // Known variant suffixes (OS/distro/edition)
    let known_variants = [
        "alpine", "slim", "debian", "bullseye", "bookworm", "buster",
        "stretch", "jessie", "oracle", "percona", "focal", "jammy",
        "noble", "windowsservercore", "nanoserver",
    ];

    let (version_part, variant) = if let Some(idx) = tag.find('-') {
        let suffix = &tag[idx + 1..];
        // Check if suffix matches a known variant or starts with a letter
        let suffix_lower = suffix.to_lowercase();
        let is_variant = known_variants.iter().any(|v| suffix_lower.starts_with(v))
            || suffix.chars().next().map_or(false, |c| c.is_alphabetic());
        if is_variant {
            (&tag[..idx], Some(suffix.to_string()))
        } else {
            (tag.as_str(), None)
        }
    } else {
        (tag.as_str(), None)
    };

    // Parse version numbers
    let parts: Vec<&str> = version_part.split('.').collect();
    let major = parts.first()?.parse::<u32>().ok()?;
    let minor = parts.get(1).and_then(|s| s.parse::<u32>().ok());
    let patch = parts.get(2).and_then(|s| s.parse::<u32>().ok());

    // If we can't parse even a major version, it's not a versioned tag
    Some(DatabaseVersion {
        major,
        minor,
        patch,
        variant,
    })
}

/// Generate pre-transfer database commands (run on source).
///
/// Produces real, actionable docker exec commands for dump + lock.
/// Version-aware: PostgreSQL ≥ 9 uses custom format, Redis ≥ 6 uses --rdb flag.
pub fn generate_db_pre_commands(
    db_type: &DatabaseType,
    service_name: &str,
    version: Option<&DatabaseVersion>,
) -> Vec<String> {
    let container = service_name;
    match db_type {
        DatabaseType::PostgreSQL => {
            let use_custom_fmt = version.map_or(true, |v| v.at_least_major(9));
            let fmt_flag = if use_custom_fmt { "Fc" } else { "Fp" };
            let dump_ext = if use_custom_fmt { "custom" } else { "sql" };
            vec![
                format!("# === PostgreSQL pre-transfer for '{}' ===", service_name),
                format!(
                    "docker exec {} psql -U postgres -c \"ALTER DATABASE postgres SET default_transaction_read_only = on;\"",
                    container
                ),
                format!(
                    "docker exec {} pg_dump -{} -f /tmp/dump.{} -U postgres -d postgres",
                    container, fmt_flag, dump_ext
                ),
                format!(
                    "docker cp {}:/tmp/dump.{} ./dump.{}",
                    container, dump_ext, dump_ext
                ),
            ]
        }
        DatabaseType::MySQL => {
            let is_mariadb = version
                .and_then(|v| v.variant.as_deref())
                .map_or(false, |var| var.contains("mariadb"));
            let dump_tool = if is_mariadb {
                "mariadb-dump"
            } else {
                "mysqldump"
            };
            vec![
                format!("# === MySQL/MariaDB pre-transfer for '{}' ===", service_name),
                format!(
                    "docker exec {} mysql -u root -e \"FLUSH TABLES WITH READ LOCK;\"",
                    container
                ),
                format!(
                    "docker exec {} sh -c \"{} --single-transaction --quick --routines --triggers --all-databases -u root > /tmp/dump.sql\"",
                    container, dump_tool
                ),
                format!("docker cp {}:/tmp/dump.sql ./dump.sql", container),
            ]
        }
        DatabaseType::MongoDB => {
            vec![
                format!("# === MongoDB pre-transfer for '{}' ===", service_name),
                format!(
                    "docker exec {} mongosh --eval \"db.fsyncLock()\"",
                    container
                ),
                format!(
                    "docker exec {} mongodump --archive=/tmp/dump.archive",
                    container
                ),
                format!(
                    "docker cp {}:/tmp/dump.archive ./dump.archive",
                    container
                ),
            ]
        }
        DatabaseType::Redis => {
            let use_rdb_flag = version.map_or(true, |v| v.at_least_major(6));
            if use_rdb_flag {
                vec![
                    format!("# === Redis pre-transfer for '{}' (>= 6) ===", service_name),
                    format!("docker exec {} redis-cli BGSAVE", container),
                    format!(
                        "docker exec {} redis-cli --rdb /tmp/dump.rdb",
                        container
                    ),
                    format!(
                        "docker cp {}:/tmp/dump.rdb ./dump.rdb",
                        container
                    ),
                ]
            } else {
                vec![
                    format!("# === Redis pre-transfer for '{}' (< 6) ===", service_name),
                    format!("docker exec {} redis-cli BGSAVE", container),
                    format!(
                        "# Wait for BGSAVE to complete: docker exec {} redis-cli LASTSAVE",
                        container
                    ),
                    format!(
                        "docker cp {}:/data/dump.rdb ./dump.rdb",
                        container
                    ),
                ]
            }
        }
        DatabaseType::Other(_) => vec![
            format!(
                "# For service '{}': review database-specific pre-transfer steps",
                service_name
            ),
        ],
    }
}

/// Generate post-transfer database commands (run on target).
///
/// Produces real, actionable docker exec commands for restore + unlock.
/// Version-aware: PostgreSQL ≥ 9 uses pg_restore for custom format.
pub fn generate_db_post_commands(
    db_type: &DatabaseType,
    service_name: &str,
    version: Option<&DatabaseVersion>,
) -> Vec<String> {
    let container = service_name;
    match db_type {
        DatabaseType::PostgreSQL => {
            let use_custom_fmt = version.map_or(true, |v| v.at_least_major(9));
            if use_custom_fmt {
                vec![
                    format!("# === PostgreSQL post-transfer for '{}' ===", service_name),
                    format!(
                        "docker cp ./dump.custom {}:/tmp/dump.custom",
                        container
                    ),
                    format!(
                        "docker exec {} pg_restore -d postgres /tmp/dump.custom",
                        container
                    ),
                    format!(
                        "docker exec {} psql -U postgres -c \"ALTER DATABASE postgres SET default_transaction_read_only = off;\"",
                        container
                    ),
                ]
            } else {
                vec![
                    format!("# === PostgreSQL post-transfer for '{}' ===", service_name),
                    format!("docker cp ./dump.sql {}:/tmp/dump.sql", container),
                    format!(
                        "docker exec {} psql -U postgres -d postgres -f /tmp/dump.sql",
                        container
                    ),
                    format!(
                        "docker exec {} psql -U postgres -c \"ALTER DATABASE postgres SET default_transaction_read_only = off;\"",
                        container
                    ),
                ]
            }
        }
        DatabaseType::MySQL => {
            vec![
                format!("# === MySQL/MariaDB post-transfer for '{}' ===", service_name),
                format!("docker cp ./dump.sql {}:/tmp/dump.sql", container),
                format!(
                    "docker exec {} sh -c \"mysql -u root < /tmp/dump.sql\"",
                    container
                ),
                format!(
                    "docker exec {} mysql -u root -e \"UNLOCK TABLES;\"",
                    container
                ),
            ]
        }
        DatabaseType::MongoDB => {
            vec![
                format!("# === MongoDB post-transfer for '{}' ===", service_name),
                format!(
                    "docker cp ./dump.archive {}:/tmp/dump.archive",
                    container
                ),
                format!(
                    "docker exec {} mongorestore --archive=/tmp/dump.archive",
                    container
                ),
                format!(
                    "docker exec {} mongosh --eval \"db.fsyncUnlock()\"",
                    container
                ),
            ]
        }
        DatabaseType::Redis => {
            vec![
                format!("# === Redis post-transfer for '{}' ===", service_name),
                format!("docker cp ./dump.rdb {}:/tmp/dump.rdb", container),
                format!(
                    "# Restore: copy dump.rdb into Redis data dir, then restart or SHUTDOWN NOSAVE: docker exec {} redis-cli SHUTDOWN NOSAVE",
                    container
                ),
            ]
        }
        DatabaseType::Other(_) => vec![
            format!(
                "# For service '{}': review database-specific post-transfer steps",
                service_name
            ),
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

    // ── E1: parse_image_version tests ──────────────────────────

    #[test]
    fn test_parse_version_full() {
        let v = parse_image_version("postgres:15.4-alpine").unwrap();
        assert_eq!(v.major, 15);
        assert_eq!(v.minor, Some(4));
        assert_eq!(v.patch, None);
        assert_eq!(v.variant.as_deref(), Some("alpine"));
    }

    #[test]
    fn test_parse_version_major_only() {
        let v = parse_image_version("postgres:15").unwrap();
        assert_eq!(v.major, 15);
        assert_eq!(v.minor, None);
        assert_eq!(v.patch, None);
        assert_eq!(v.variant, None);
    }

    #[test]
    fn test_parse_version_latest() {
        assert_eq!(parse_image_version("postgres:latest"), None);
    }

    #[test]
    fn test_parse_version_mariadb() {
        let v = parse_image_version("mariadb:10.11").unwrap();
        assert_eq!(v.major, 10);
        assert_eq!(v.minor, Some(11));
        assert_eq!(v.variant, None);
    }

    #[test]
    fn test_parse_version_mysql_debian() {
        let v = parse_image_version("mysql:8.0.36-debian").unwrap();
        assert_eq!(v.major, 8);
        assert_eq!(v.minor, Some(0));
        assert_eq!(v.patch, Some(36));
        assert_eq!(v.variant.as_deref(), Some("debian"));
    }

    #[test]
    fn test_parse_version_redis_alpine() {
        let v = parse_image_version("redis:7-alpine").unwrap();
        assert_eq!(v.major, 7);
        assert_eq!(v.minor, None);
        assert_eq!(v.variant.as_deref(), Some("alpine"));
    }

    #[test]
    fn test_parse_version_no_tag() {
        assert_eq!(parse_image_version("nginx"), None);
    }

    // ── E2: PostgreSQL pre/post command tests ──────────────────

    #[test]
    fn test_postgres_pre_commands_custom_format() {
        let v = parse_image_version("postgres:15.4").unwrap();
        let cmds = generate_db_pre_commands(&DatabaseType::PostgreSQL, "mydb", Some(&v));
        // Should use custom format (PG >= 9)
        assert!(cmds.iter().any(|c| c.contains("pg_dump -Fc")));
        assert!(cmds.iter().any(|c| c.contains("dump.custom")));
        assert!(cmds.iter().any(|c| c.contains("read_only")));
        assert!(!cmds.iter().any(|c| c.contains("pg_dump -Fp")));
    }

    #[test]
    fn test_postgres_pre_commands_plain_format() {
        // Old postgres version (< 9) forces plain format
        let v = DatabaseVersion {
            major: 8,
            minor: Some(4),
            patch: None,
            variant: None,
        };
        let cmds = generate_db_pre_commands(&DatabaseType::PostgreSQL, "olddb", Some(&v));
        assert!(cmds.iter().any(|c| c.contains("pg_dump -Fp")));
        assert!(cmds.iter().any(|c| c.contains("dump.sql")));
    }

    #[test]
    fn test_postgres_post_commands_restore() {
        let v = parse_image_version("postgres:15").unwrap();
        let cmds = generate_db_post_commands(&DatabaseType::PostgreSQL, "mydb", Some(&v));
        assert!(cmds.iter().any(|c| c.contains("pg_restore")));
        assert!(cmds.iter().any(|c| c.contains("read_only = off")));
    }

    // ── E3: MySQL pre/post command tests ───────────────────────

    #[test]
    fn test_mysql_pre_commands() {
        let v = parse_image_version("mysql:8.0").unwrap();
        let cmds = generate_db_pre_commands(&DatabaseType::MySQL, "mysqlsvc", Some(&v));
        assert!(cmds.iter().any(|c| c.contains("FLUSH TABLES WITH READ LOCK")));
        assert!(cmds.iter().any(|c| c.contains("mysqldump")));
        assert!(cmds.iter().any(|c| c.contains("--single-transaction")));
        assert!(cmds.iter().any(|c| c.contains("--routines")));
        assert!(cmds.iter().any(|c| c.contains("--triggers")));
    }

    #[test]
    fn test_mariadb_uses_mariadb_dump() {
        // simulate a mariadb variant
        let v = DatabaseVersion {
            major: 10,
            minor: Some(11),
            patch: None,
            variant: Some("mariadb".to_string()),
        };
        let cmds = generate_db_pre_commands(&DatabaseType::MySQL, "mariadb", Some(&v));
        assert!(cmds.iter().any(|c| c.contains("mariadb-dump")));
        assert!(!cmds.iter().any(|c| c.contains("mysqldump")));
    }

    #[test]
    fn test_mysql_post_commands() {
        let cmds = generate_db_post_commands(&DatabaseType::MySQL, "mysqlsvc", None);
        assert!(cmds.iter().any(|c| c.contains("mysql -u root < /tmp/dump.sql")));
        assert!(cmds.iter().any(|c| c.contains("UNLOCK TABLES")));
    }

    // ── E4: MongoDB + Redis command tests ──────────────────────

    #[test]
    fn test_mongodb_pre_commands() {
        let cmds = generate_db_pre_commands(&DatabaseType::MongoDB, "mongo", None);
        assert!(cmds.iter().any(|c| c.contains("db.fsyncLock()")));
        assert!(cmds.iter().any(|c| c.contains("mongodump --archive=/tmp/dump.archive")));
    }

    #[test]
    fn test_mongodb_post_commands() {
        let cmds = generate_db_post_commands(&DatabaseType::MongoDB, "mongo", None);
        assert!(cmds.iter().any(|c| c.contains("mongorestore --archive=/tmp/dump.archive")));
        assert!(cmds.iter().any(|c| c.contains("db.fsyncUnlock()")));
    }

    #[test]
    fn test_redis_pre_commands_rdb_flag() {
        let v = parse_image_version("redis:7-alpine").unwrap();
        let cmds = generate_db_pre_commands(&DatabaseType::Redis, "cache", Some(&v));
        assert!(cmds.iter().any(|c| c.contains("redis-cli --rdb")));
        assert!(cmds.iter().any(|c| c.contains("BGSAVE")));
    }

    #[test]
    fn test_redis_pre_commands_legacy() {
        let v = DatabaseVersion {
            major: 5,
            minor: None,
            patch: None,
            variant: None,
        };
        let cmds = generate_db_pre_commands(&DatabaseType::Redis, "oldcache", Some(&v));
        assert!(cmds.iter().any(|c| c.contains("BGSAVE")));
        assert!(cmds.iter().any(|c| c.contains("/data/dump.rdb")));
        assert!(!cmds.iter().any(|c| c.contains("--rdb")));
    }

    #[test]
    fn test_redis_post_commands() {
        let cmds = generate_db_post_commands(&DatabaseType::Redis, "cache", None);
        assert!(cmds.iter().any(|c| c.contains("dump.rdb")));
        assert!(cmds.iter().any(|c| c.contains("SHUTDOWN NOSAVE")));
    }

    // ── DatabaseVersion methods ───────────────────────────────

    #[test]
    fn test_db_version_at_least() {
        let v = DatabaseVersion {
            major: 9,
            minor: Some(6),
            patch: None,
            variant: None,
        };
        assert!(v.at_least(9, 0));
        assert!(v.at_least(9, 5));
        assert!(!v.at_least(9, 7));
        assert!(!v.at_least(10, 0));
    }

    #[test]
    fn test_db_version_at_least_major() {
        let v = DatabaseVersion {
            major: 6,
            minor: Some(2),
            patch: None,
            variant: None,
        };
        assert!(v.at_least_major(6));
        assert!(v.at_least_major(5));
        assert!(!v.at_least_major(7));
    }

    // ── Integration: version-aware commands in diff output ────

    #[test]
    fn test_diff_postgres_version_aware() {
        let src = "services:\n  db:\n    image: postgres:15.4-alpine\n";
        let diff = diff_compose(src, src, None, None).unwrap();
        assert_eq!(diff.database_services.len(), 1);
        let db = &diff.database_services[0];
        assert_eq!(db.db_type, DatabaseType::PostgreSQL);
        assert_eq!(db.version.as_deref(), Some("15.4-alpine"));
        // Pre commands should use custom format (PG >= 9)
        assert!(db.pre_transfer_commands.iter().any(|c| c.contains("pg_dump -Fc")));
        // Post commands should use pg_restore
        assert!(db
            .post_transfer_commands
            .iter()
            .any(|c| c.contains("pg_restore")));
    }

    #[test]
    fn test_diff_mongodb_commands() {
        let src = "services:\n  mongo:\n    image: mongo:7\n";
        let diff = diff_compose(src, src, None, None).unwrap();
        assert_eq!(diff.database_services.len(), 1);
        let db = &diff.database_services[0];
        assert_eq!(db.db_type, DatabaseType::MongoDB);
        assert!(db.pre_transfer_commands.iter().any(|c| c.contains("mongodump")));
        assert!(db
            .post_transfer_commands
            .iter()
            .any(|c| c.contains("mongorestore")));
    }
}
