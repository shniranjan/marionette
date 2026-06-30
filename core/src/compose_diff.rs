// ── Compose Template Diff Engine ─────────────────────────────────
// Parses two Docker Compose YAMLs and produces a structured diff
// suitable for migration planning. Handles volume changes, service
// changes, image updates, env var diffs, port mappings, architecture
// warnings, and database service detection.

use serde_yaml::Value;

/// Structured diff between two compose files.
#[deprecated(note = "Use unified_migration::MigrationPlan")]
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

#[deprecated(note = "Use unified_migration::UnifiedVolume")]
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

#[deprecated(note = "Use unified_migration::UnifiedEnvVar")]
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

#[deprecated(note = "Use unified_migration::UnifiedDatabase")]
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseService {
    pub service_name: String,
    pub db_type: DatabaseType,
    pub image: String,
    pub version: Option<String>,
    pub has_replication: bool,
    pub username: Option<String>,
    pub password_masked: Option<String>,
    pub port: Option<String>,
    pub database_name: Option<String>,
    pub pre_transfer_commands: Vec<String>,
    pub post_transfer_commands: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
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
    /// True if this version is at least the given major version.
    pub fn at_least_major(&self, major: u32) -> bool {
        self.major >= major
    }
}

/// Extracted database credentials and connection parameters from compose env.
#[derive(Debug, Clone, Default)]
pub(crate) struct DatabaseCredentials {
    username: Option<String>,
    password: Option<String>,
    port: Option<String>,
    database_name: Option<String>,
}

/// Detect database credentials from compose environment variables.
fn detect_db_credentials(
    db_type: &DatabaseType,
    env: &std::collections::HashMap<String, String>,
    ports_yaml: Option<&Value>,
) -> DatabaseCredentials {
    let mut creds = DatabaseCredentials::default();

    // Check ports section for standard database port mappings
    let port_from_ports = detect_port_from_ports_section(ports_yaml);
    if port_from_ports.is_some() {
        creds.port = port_from_ports;
    }

    match db_type {
        DatabaseType::PostgreSQL => {
            creds.username = env
                .get("POSTGRES_USER")
                .or_else(|| env.get("PGUSER"))
                .cloned();
            creds.password = env
                .get("POSTGRES_PASSWORD")
                .or_else(|| env.get("PGPASSWORD"))
                .cloned();
            creds.database_name = env.get("POSTGRES_DB").cloned();
            // Check PGPORT env var (override if present)
            if let Some(pgport) = env.get("PGPORT") {
                creds.port = Some(pgport.clone());
            }
        }
        DatabaseType::MySQL => {
            creds.username = env
                .get("MYSQL_USER")
                .cloned();
            creds.password = env
                .get("MYSQL_ROOT_PASSWORD")
                .or_else(|| env.get("MYSQL_PASSWORD"))
                .cloned();
            creds.database_name = env.get("MYSQL_DATABASE").cloned();
            // Check MYSQL_TCP_PORT env var (override if present)
            if let Some(mysql_port) = env.get("MYSQL_TCP_PORT") {
                creds.port = Some(mysql_port.clone());
            }
        }
        DatabaseType::MongoDB => {
            creds.username = env
                .get("MONGO_INITDB_ROOT_USERNAME")
                .or_else(|| env.get("MONGO_USER"))
                .cloned();
            creds.password = env
                .get("MONGO_INITDB_ROOT_PASSWORD")
                .or_else(|| env.get("MONGO_PASSWORD"))
                .cloned();
            creds.database_name = env.get("MONGO_INITDB_DATABASE").cloned();
            if let Some(mongo_port) = env.get("MONGO_PORT") {
                creds.port = Some(mongo_port.clone());
            }
        }
        DatabaseType::Redis => {
            creds.password = env.get("REDIS_PASSWORD").cloned();
            if let Some(redis_port) = env.get("REDIS_PORT") {
                creds.port = Some(redis_port.clone());
            }
        }
        DatabaseType::Other(_) => {}
    }

    creds
}

/// Detect exposed port from compose ports section (e.g., "5432:5432" → container port 5432).
fn detect_port_from_ports_section(ports: Option<&Value>) -> Option<String> {
    let seq = ports?.as_sequence()?;
    for item in seq {
        let s = item.as_str()?;
        // Format: "HOST:CONTAINER" or "CONTAINER"
        let container_port = if let Some((_host, container)) = s.split_once(':') {
            container
        } else {
            s
        };
        // Check for standard database ports
        match container_port {
            "5432" | "3306" | "27017" | "6379" | "16379" => return Some(container_port.to_string()),
            _ => {}
        }
    }
    None
}

/// Mask a password for display (show first char + asterisks).
fn mask_password(password: &str) -> String {
    if password.is_empty() {
        return String::new();
    }
    let mut masked = String::with_capacity(password.len());
    masked.push(password.chars().next().unwrap_or('*'));
    for _ in 1..password.len() {
        masked.push('*');
    }
    masked
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
                        let env_map = env_to_map(details.get("environment"));
                        let creds = detect_db_credentials(&db_type, &env_map, details.get("ports"));
                        let password_masked = creds.password.as_ref().map(|p| mask_password(p));
                        diff.database_services.push(DatabaseService {
                            service_name: name_str.to_string(),
                            db_type: db_type.clone(),
                            image: image.to_string(),
                            version: img_version(image),
                            has_replication: false,
                            username: creds.username.clone(),
                            password_masked: password_masked,
                            port: creds.port.clone(),
                            database_name: creds.database_name.clone(),
                            pre_transfer_commands: generate_db_pre_commands(
                                &db_type,
                                name_str,
                                parsed_version.as_ref(),
                                &creds,
                            ),
                            post_transfer_commands: generate_db_post_commands(
                                &db_type,
                                name_str,
                                parsed_version.as_ref(),
                                &creds,
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
/// Credential-aware: injects username/password from compose environment.
/// Port-aware: uses custom port if detected.
/// Database-targeting: dumps specific DB if POSTGRES_DB/MYSQL_DATABASE/etc. is set.
pub fn generate_db_pre_commands(
    db_type: &DatabaseType,
    service_name: &str,
    version: Option<&DatabaseVersion>,
    creds: &DatabaseCredentials,
) -> Vec<String> {
    let container = service_name;
    match db_type {
        DatabaseType::PostgreSQL => {
            let use_custom_fmt = version.map_or(true, |v| v.at_least_major(9));
            let fmt_flag = if use_custom_fmt { "Fc" } else { "Fp" };
            let dump_ext = if use_custom_fmt { "custom" } else { "sql" };
            let user = creds.username.as_deref().unwrap_or("postgres");
            let db = creds.database_name.as_deref().unwrap_or("postgres");
            let password_prefix = creds
                .password
                .as_ref()
                .map(|p| format!("PGPASSWORD={} ", p))
                .unwrap_or_default();
            let port_flag = creds
                .port
                .as_ref()
                .map(|p| format!(" -p {}", p))
                .unwrap_or_default();

            let mut cmds = vec![
                format!("# === PostgreSQL pre-transfer for '{}' ===", service_name),
                format!(
                    "docker exec {} {}psql -U {}{} -c \"ALTER DATABASE {} SET default_transaction_read_only = on;\"",
                    container, password_prefix, user, port_flag, db
                ),
                format!(
                    "docker exec {} {}pg_dump -{} -f /tmp/dump.{} -U {}{} -d {}",
                    container, password_prefix, fmt_flag, dump_ext, user, port_flag, db
                ),
                format!(
                    "docker cp {}:/tmp/dump.{} ./dump.{}",
                    container, dump_ext, dump_ext
                ),
            ];

            // Add pg_dumpall for globals/roles (PG >= 9)
            if version.map_or(true, |v| v.at_least_major(9)) {
                cmds.push(format!(
                    "docker exec {} {}pg_dumpall -U {}{} -g -f /tmp/globals.sql",
                    container, password_prefix, user, port_flag
                ));
                cmds.push(format!(
                    "docker cp {}:/tmp/globals.sql ./globals.sql",
                    container
                ));
            }

            cmds
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
            let user = creds.username.as_deref().unwrap_or("root");
            let password_flag = creds
                .password
                .as_ref()
                .map(|p| format!("-p{} ", p))
                .unwrap_or_default();
            let port_flag = creds
                .port
                .as_ref()
                .map(|p| format!(" -P {}", p))
                .unwrap_or_default();
            let db_target = creds
                .database_name
                .as_deref()
                .map(|db| format!(" {}", db))
                .unwrap_or_else(|| " --all-databases".to_string());

            vec![
                format!("# === MySQL/MariaDB pre-transfer for '{}' ===", service_name),
                format!(
                    "docker exec {} mysql -u {}{}{} -e \"FLUSH TABLES WITH READ LOCK;\"",
                    container, user, password_flag, port_flag
                ),
                format!(
                    "docker exec {} sh -c \"{} --single-transaction --quick --routines --triggers{} -u {}{}{} > /tmp/dump.sql\"",
                    container, dump_tool, db_target, user, password_flag, port_flag
                ),
                format!("docker cp {}:/tmp/dump.sql ./dump.sql", container),
            ]
        }
        DatabaseType::MongoDB => {
            let user_flag = creds
                .username
                .as_ref()
                .map(|u| format!(" -u {}", u))
                .unwrap_or_default();
            let password_flag = creds
                .password
                .as_ref()
                .map(|p| format!(" -p {}", p))
                .unwrap_or_default();
            let port_flag = creds
                .port
                .as_ref()
                .map(|p| format!(" --port {}", p))
                .unwrap_or_default();
            let db_flag = creds
                .database_name
                .as_ref()
                .map(|db| format!(" {}", db))
                .unwrap_or_default();

            vec![
                format!("# === MongoDB pre-transfer for '{}' ===", service_name),
                format!(
                    "docker exec {} mongosh{} --eval \"db = db.getSiblingDB('{}'); db.fsyncLock()\"{}",
                    container, user_flag,
                    creds.database_name.as_deref().unwrap_or("admin"),
                    password_flag
                ),
                format!(
                    "docker exec {} mongodump --archive=/tmp/dump.archive{}{}{}{}",
                    container, user_flag, password_flag, port_flag, db_flag
                ),
                format!(
                    "docker cp {}:/tmp/dump.archive ./dump.archive",
                    container
                ),
            ]
        }
        DatabaseType::Redis => {
            let use_rdb_flag = version.map_or(true, |v| v.at_least_major(6));
            let auth_flag = creds
                .password
                .as_ref()
                .map(|p| format!(" -a {}", p))
                .unwrap_or_default();
            let port_flag = creds
                .port
                .as_ref()
                .map(|p| format!(" -p {}", p))
                .unwrap_or_default();

            if use_rdb_flag {
                vec![
                    format!("# === Redis pre-transfer for '{}' (>= 6) ===", service_name),
                    format!(
                        "docker exec {} redis-cli{}{} BGSAVE",
                        container, auth_flag, port_flag
                    ),
                    format!(
                        "docker exec {} redis-cli{}{} --rdb /tmp/dump.rdb",
                        container, auth_flag, port_flag
                    ),
                    format!(
                        "docker cp {}:/tmp/dump.rdb ./dump.rdb",
                        container
                    ),
                ]
            } else {
                vec![
                    format!("# === Redis pre-transfer for '{}' (< 6) ===", service_name),
                    format!(
                        "docker exec {} redis-cli{}{} BGSAVE",
                        container, auth_flag, port_flag
                    ),
                    format!(
                        "# Wait for BGSAVE to complete: docker exec {} redis-cli{}{} LASTSAVE",
                        container, auth_flag, port_flag
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
/// Credential-aware: injects username/password from compose environment.
/// Port-aware: uses custom port if detected.
pub fn generate_db_post_commands(
    db_type: &DatabaseType,
    service_name: &str,
    version: Option<&DatabaseVersion>,
    creds: &DatabaseCredentials,
) -> Vec<String> {
    let container = service_name;
    match db_type {
        DatabaseType::PostgreSQL => {
            let use_custom_fmt = version.map_or(true, |v| v.at_least_major(9));
            let user = creds.username.as_deref().unwrap_or("postgres");
            let db = creds.database_name.as_deref().unwrap_or("postgres");
            let password_prefix = creds
                .password
                .as_ref()
                .map(|p| format!("PGPASSWORD={} ", p))
                .unwrap_or_default();
            let port_flag = creds
                .port
                .as_ref()
                .map(|p| format!(" -p {}", p))
                .unwrap_or_default();

            let mut cmds = if use_custom_fmt {
                vec![
                    format!("# === PostgreSQL post-transfer for '{}' ===", service_name),
                    format!(
                        "docker cp ./dump.custom {}:/tmp/dump.custom",
                        container
                    ),
                    format!(
                        "docker exec {} {}pg_restore -U {}{} -d {} /tmp/dump.custom",
                        container, password_prefix, user, port_flag, db
                    ),
                ]
            } else {
                vec![
                    format!("# === PostgreSQL post-transfer for '{}' ===", service_name),
                    format!("docker cp ./dump.sql {}:/tmp/dump.sql", container),
                    format!(
                        "docker exec {} {}psql -U {}{} -d {} -f /tmp/dump.sql",
                        container, password_prefix, user, port_flag, db
                    ),
                ]
            };

            // Restore globals/roles if pg_dumpall was used (PG >= 9)
            if version.map_or(true, |v| v.at_least_major(9)) {
                cmds.push(format!(
                    "docker cp ./globals.sql {}:/tmp/globals.sql",
                    container
                ));
                cmds.push(format!(
                    "docker exec {} {}psql -U {}{} -d {} -f /tmp/globals.sql",
                    container, password_prefix, user, port_flag, db
                ));
            }

            cmds.push(format!(
                "docker exec {} {}psql -U {}{} -c \"ALTER DATABASE {} SET default_transaction_read_only = off;\"",
                container, password_prefix, user, port_flag, db
            ));

            cmds
        }
        DatabaseType::MySQL => {
            let user = creds.username.as_deref().unwrap_or("root");
            let password_flag = creds
                .password
                .as_ref()
                .map(|p| format!("-p{} ", p))
                .unwrap_or_default();
            let port_flag = creds
                .port
                .as_ref()
                .map(|p| format!(" -P {}", p))
                .unwrap_or_default();

            vec![
                format!("# === MySQL/MariaDB post-transfer for '{}' ===", service_name),
                format!("docker cp ./dump.sql {}:/tmp/dump.sql", container),
                format!(
                    "docker exec {} sh -c \"mysql -u {}{}{} < /tmp/dump.sql\"",
                    container, user, password_flag, port_flag
                ),
                format!(
                    "docker exec {} mysql -u {}{}{} -e \"UNLOCK TABLES;\"",
                    container, user, password_flag, port_flag
                ),
            ]
        }
        DatabaseType::MongoDB => {
            let user_flag = creds
                .username
                .as_ref()
                .map(|u| format!(" -u {}", u))
                .unwrap_or_default();
            let password_flag = creds
                .password
                .as_ref()
                .map(|p| format!(" -p {}", p))
                .unwrap_or_default();
            let port_flag = creds
                .port
                .as_ref()
                .map(|p| format!(" --port {}", p))
                .unwrap_or_default();

            vec![
                format!("# === MongoDB post-transfer for '{}' ===", service_name),
                format!(
                    "docker cp ./dump.archive {}:/tmp/dump.archive",
                    container
                ),
                format!(
                    "docker exec {} mongorestore --archive=/tmp/dump.archive{}{}{}",
                    container, user_flag, password_flag, port_flag
                ),
                format!(
                    "docker exec {} mongosh{} --eval \"db = db.getSiblingDB('{}'); db.fsyncUnlock()\"{}",
                    container, user_flag,
                    creds.database_name.as_deref().unwrap_or("admin"),
                    password_flag
                ),
            ]
        }
        DatabaseType::Redis => {
            let auth_flag = creds
                .password
                .as_ref()
                .map(|p| format!(" -a {}", p))
                .unwrap_or_default();
            let port_flag = creds
                .port
                .as_ref()
                .map(|p| format!(" -p {}", p))
                .unwrap_or_default();

            vec![
                format!("# === Redis post-transfer for '{}' ===", service_name),
                format!("docker cp ./dump.rdb {}:/tmp/dump.rdb", container),
                format!(
                    "# Restore: copy dump.rdb into Redis data dir, then restart or SHUTDOWN NOSAVE: docker exec {} redis-cli{}{} SHUTDOWN NOSAVE",
                    container, auth_flag, port_flag
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
        let creds = DatabaseCredentials::default();
        let cmds = generate_db_pre_commands(&DatabaseType::PostgreSQL, "mydb", Some(&v), &creds);
        // Should use custom format (PG >= 9)
        assert!(cmds.iter().any(|c| c.contains("pg_dump -Fc")));
        assert!(cmds.iter().any(|c| c.contains("dump.custom")));
        assert!(cmds.iter().any(|c| c.contains("read_only")));
        assert!(!cmds.iter().any(|c| c.contains("pg_dump -Fp")));
        // PG >= 9 should include pg_dumpall for globals
        assert!(cmds.iter().any(|c| c.contains("pg_dumpall")));
        assert!(cmds.iter().any(|c| c.contains("globals.sql")));
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
        let creds = DatabaseCredentials::default();
        let cmds = generate_db_pre_commands(&DatabaseType::PostgreSQL, "olddb", Some(&v), &creds);
        assert!(cmds.iter().any(|c| c.contains("pg_dump -Fp")));
        assert!(cmds.iter().any(|c| c.contains("dump.sql")));
        // PG < 9 should NOT have pg_dumpall
        assert!(!cmds.iter().any(|c| c.contains("pg_dumpall")));
    }

    #[test]
    fn test_postgres_post_commands_restore() {
        let v = parse_image_version("postgres:15").unwrap();
        let creds = DatabaseCredentials::default();
        let cmds = generate_db_post_commands(&DatabaseType::PostgreSQL, "mydb", Some(&v), &creds);
        assert!(cmds.iter().any(|c| c.contains("pg_restore")));
        assert!(cmds.iter().any(|c| c.contains("read_only = off")));
        // PG >= 9 should include globals.sql restore
        assert!(cmds.iter().any(|c| c.contains("globals.sql")));
    }

    // ── E3: MySQL pre/post command tests ───────────────────────

    #[test]
    fn test_mysql_pre_commands() {
        let v = parse_image_version("mysql:8.0").unwrap();
        let creds = DatabaseCredentials::default();
        let cmds = generate_db_pre_commands(&DatabaseType::MySQL, "mysqlsvc", Some(&v), &creds);
        assert!(cmds.iter().any(|c| c.contains("FLUSH TABLES WITH READ LOCK")));
        assert!(cmds.iter().any(|c| c.contains("mysqldump")));
        assert!(cmds.iter().any(|c| c.contains("--single-transaction")));
        assert!(cmds.iter().any(|c| c.contains("--routines")));
        assert!(cmds.iter().any(|c| c.contains("--triggers")));
        // Default: no creds → expects --all-databases
        assert!(cmds.iter().any(|c| c.contains("--all-databases")));
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
        let creds = DatabaseCredentials::default();
        let cmds = generate_db_pre_commands(&DatabaseType::MySQL, "mariadb", Some(&v), &creds);
        assert!(cmds.iter().any(|c| c.contains("mariadb-dump")));
        assert!(!cmds.iter().any(|c| c.contains("mysqldump")));
    }

    #[test]
    fn test_mysql_post_commands() {
        let creds = DatabaseCredentials::default();
        let cmds = generate_db_post_commands(&DatabaseType::MySQL, "mysqlsvc", None, &creds);
        assert!(cmds.iter().any(|c| c.contains("mysql -u root < /tmp/dump.sql")));
        assert!(cmds.iter().any(|c| c.contains("UNLOCK TABLES")));
    }

    // ── E4: MongoDB + Redis command tests ──────────────────────

    #[test]
    fn test_mongodb_pre_commands() {
        let creds = DatabaseCredentials::default();
        let cmds = generate_db_pre_commands(&DatabaseType::MongoDB, "mongo", None, &creds);
        assert!(cmds.iter().any(|c| c.contains("db.fsyncLock()")));
        assert!(cmds.iter().any(|c| c.contains("mongodump --archive=/tmp/dump.archive")));
    }

    #[test]
    fn test_mongodb_post_commands() {
        let creds = DatabaseCredentials::default();
        let cmds = generate_db_post_commands(&DatabaseType::MongoDB, "mongo", None, &creds);
        assert!(cmds.iter().any(|c| c.contains("mongorestore --archive=/tmp/dump.archive")));
        assert!(cmds.iter().any(|c| c.contains("db.fsyncUnlock()")));
    }

    #[test]
    fn test_redis_pre_commands_rdb_flag() {
        let v = parse_image_version("redis:7-alpine").unwrap();
        let creds = DatabaseCredentials::default();
        let cmds = generate_db_pre_commands(&DatabaseType::Redis, "cache", Some(&v), &creds);
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
        let creds = DatabaseCredentials::default();
        let cmds = generate_db_pre_commands(&DatabaseType::Redis, "oldcache", Some(&v), &creds);
        assert!(cmds.iter().any(|c| c.contains("BGSAVE")));
        assert!(cmds.iter().any(|c| c.contains("/data/dump.rdb")));
        assert!(!cmds.iter().any(|c| c.contains("--rdb")));
    }

    #[test]
    fn test_redis_post_commands() {
        let creds = DatabaseCredentials::default();
        let cmds = generate_db_post_commands(&DatabaseType::Redis, "cache", None, &creds);
        assert!(cmds.iter().any(|c| c.contains("dump.rdb")));
        assert!(cmds.iter().any(|c| c.contains("SHUTDOWN NOSAVE")));
    }

    // ── DatabaseVersion methods ───────────────────────────────

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
        assert!(db.post_transfer_commands.iter().any(|c| c.contains("pg_restore")));
        // New fields should be None (no env set)
        assert_eq!(db.username, None);
        assert_eq!(db.password_masked, None);
        assert_eq!(db.port, None);
        assert_eq!(db.database_name, None);
    }

    #[test]
    fn test_diff_mongodb_commands() {
        let src = "services:\n  mongo:\n    image: mongo:7\n";
        let diff = diff_compose(src, src, None, None).unwrap();
        assert_eq!(diff.database_services.len(), 1);
        let db = &diff.database_services[0];
        assert_eq!(db.db_type, DatabaseType::MongoDB);
        assert!(db.pre_transfer_commands.iter().any(|c| c.contains("mongodump")));
        assert!(db.post_transfer_commands.iter().any(|c| c.contains("mongorestore")));
    }

    // ── Phase E: Credential detection tests ───────────────────

    #[test]
    fn test_detect_postgres_with_env_credentials() {
        let src = "services:\n  db:\n    image: postgres:15\n    environment:\n      POSTGRES_USER: myuser\n      POSTGRES_PASSWORD: secret123\n      POSTGRES_DB: myapp\n";
        let diff = diff_compose(src, src, None, None).unwrap();
        assert_eq!(diff.database_services.len(), 1);
        let db = &diff.database_services[0];
        assert_eq!(db.db_type, DatabaseType::PostgreSQL);
        assert_eq!(db.username.as_deref(), Some("myuser"));
        assert!(db.password_masked.as_ref().unwrap().starts_with('s'));
        assert!(db.password_masked.as_ref().unwrap().contains('*'));
        assert_eq!(db.database_name.as_deref(), Some("myapp"));
        // Commands should reference the custom user and database
        assert!(db.pre_transfer_commands.iter().any(|c| c.contains("-U myuser")));
        assert!(db.pre_transfer_commands.iter().any(|c| c.contains("-d myapp")));
        assert!(db.pre_transfer_commands.iter().any(|c| c.contains("PGPASSWORD=secret123")));
    }

    #[test]
    fn test_detect_mysql_with_env_credentials() {
        let src = "services:\n  db:\n    image: mysql:8.0\n    environment:\n      MYSQL_USER: dbuser\n      MYSQL_ROOT_PASSWORD: rootpass\n      MYSQL_DATABASE: mydb\n";
        let diff = diff_compose(src, src, None, None).unwrap();
        assert_eq!(diff.database_services.len(), 1);
        let db = &diff.database_services[0];
        assert_eq!(db.username.as_deref(), Some("dbuser"));
        assert!(db.password_masked.is_some());
        assert_eq!(db.database_name.as_deref(), Some("mydb"));
        // Commands should use -u dbuser and NOT --all-databases (specific DB targeting)
        assert!(db.pre_transfer_commands.iter().any(|c| c.contains("-u dbuser")));
        assert!(db.pre_transfer_commands.iter().any(|c| c.contains("-prootpass")));
        assert!(db.pre_transfer_commands.iter().any(|c| c.contains(" mydb ")));
        assert!(!db.pre_transfer_commands.iter().any(|c| c.contains("--all-databases")));
    }

    #[test]
    fn test_mysql_custom_port_detection() {
        let src = "services:\n  db:\n    image: mysql:8.0\n    environment:\n      MYSQL_TCP_PORT: '3307'\n    ports:\n      - '3307:3306'\n";
        let diff = diff_compose(src, src, None, None).unwrap();
        assert_eq!(diff.database_services.len(), 1);
        let db = &diff.database_services[0];
        // MYSQL_TCP_PORT env var should override port from ports section
        assert_eq!(db.port.as_deref(), Some("3307"));
        // Commands should include -P 3307
        assert!(db.pre_transfer_commands.iter().any(|c| c.contains("-P 3307")));
    }

    #[test]
    fn test_postgres_pgport_env_detection() {
        let src = "services:\n  db:\n    image: postgres:15\n    environment:\n      PGPORT: '5433'\n";
        let diff = diff_compose(src, src, None, None).unwrap();
        assert_eq!(diff.database_services.len(), 1);
        let db = &diff.database_services[0];
        assert_eq!(db.port.as_deref(), Some("5433"));
        assert!(db.pre_transfer_commands.iter().any(|c| c.contains("-p 5433")));
    }

    #[test]
    fn test_mongodb_auth_commands() {
        let src = "services:\n  mongo:\n    image: mongo:7\n    environment:\n      MONGO_INITDB_ROOT_USERNAME: admin\n      MONGO_INITDB_ROOT_PASSWORD: mpass\n      MONGO_INITDB_DATABASE: appdb\n";
        let diff = diff_compose(src, src, None, None).unwrap();
        assert_eq!(diff.database_services.len(), 1);
        let db = &diff.database_services[0];
        assert_eq!(db.username.as_deref(), Some("admin"));
        assert!(db.password_masked.is_some());
        assert_eq!(db.database_name.as_deref(), Some("appdb"));
        // Commands should contain auth flags
        assert!(db.pre_transfer_commands.iter().any(|c| c.contains("-u admin")));
        assert!(db.pre_transfer_commands.iter().any(|c| c.contains("-p mpass")));
        assert!(db.pre_transfer_commands.iter().any(|c| c.contains("appdb")));
    }

    #[test]
    fn test_redis_auth_commands() {
        let src = "services:\n  cache:\n    image: redis:7\n    environment:\n      REDIS_PASSWORD: redissecret\n      REDIS_PORT: '6380'\n";
        let diff = diff_compose(src, src, None, None).unwrap();
        assert_eq!(diff.database_services.len(), 1);
        let db = &diff.database_services[0];
        assert_eq!(db.port.as_deref(), Some("6380"));
        assert!(db.password_masked.is_some());
        // Commands should have -a flag for auth and -p for port
        assert!(db.pre_transfer_commands.iter().any(|c| c.contains("-a redissecret")));
        assert!(db.pre_transfer_commands.iter().any(|c| c.contains("-p 6380")));
    }

    #[test]
    fn test_database_service_new_fields_serialization() {
        let ds = DatabaseService {
            service_name: "testdb".to_string(),
            db_type: DatabaseType::PostgreSQL,
            image: "postgres:15".to_string(),
            version: Some("15".to_string()),
            has_replication: false,
            username: Some("myuser".to_string()),
            password_masked: Some("s*********".to_string()),
            port: Some("5432".to_string()),
            database_name: Some("mydb".to_string()),
            pre_transfer_commands: vec!["cmd1".to_string()],
            post_transfer_commands: vec!["cmd2".to_string()],
        };
        let json = serde_json::to_string(&ds).unwrap();
        // Verify camelCase JSON keys
        assert!(json.contains("\"serviceName\""));
        assert!(json.contains("\"dbType\""));
        assert!(json.contains("\"hasReplication\""));
        assert!(json.contains("\"username\""));
        assert!(json.contains("\"passwordMasked\""));
        assert!(json.contains("\"port\""));
        assert!(json.contains("\"databaseName\""));
        assert!(json.contains("\"preTransferCommands\""));
        assert!(json.contains("\"postTransferCommands\""));
        // Verify values
        assert!(json.contains("\"myuser\""));
        assert!(json.contains("\"s*********\""));
        assert!(json.contains("\"5432\""));
        assert!(json.contains("\"mydb\""));
    }

    #[test]
    fn test_port_detection_from_ports_section() {
        // Test that ports section is detected without env vars
        let src = "services:\n  db:\n    image: postgres:15\n    ports:\n      - '5432:5432'\n";
        let diff = diff_compose(src, src, None, None).unwrap();
        assert_eq!(diff.database_services.len(), 1);
        let db = &diff.database_services[0];
        assert_eq!(db.port.as_deref(), Some("5432"));
    }

    #[test]
    fn test_pg_dumpall_includes_globals_dump() {
        let src = "services:\n  db:\n    image: postgres:12\n    environment:\n      POSTGRES_PASSWORD: pgpass\n";
        let diff = diff_compose(src, src, None, None).unwrap();
        assert_eq!(diff.database_services.len(), 1);
        let db = &diff.database_services[0];
        // Should have pg_dumpall -g for globals
        assert!(db.pre_transfer_commands.iter().any(|c| c.contains("pg_dumpall") && c.contains("-g")));
        assert!(db.pre_transfer_commands.iter().any(|c| c.contains("globals.sql")));
        // Post commands should restore globals
        assert!(db.post_transfer_commands.iter().any(|c| c.contains("globals.sql")));
    }

    #[test]
    fn test_mask_password_function() {
        assert_eq!(mask_password("secret123"), "s********");
        assert_eq!(mask_password("a"), "a");
        assert_eq!(mask_password(""), "");
    }

    #[test]
    fn test_detect_db_credentials_empty() {
        let env = std::collections::HashMap::new();
        let creds = detect_db_credentials(&DatabaseType::PostgreSQL, &env, None);
        assert_eq!(creds.username, None);
        assert_eq!(creds.password, None);
        assert_eq!(creds.port, None);
        assert_eq!(creds.database_name, None);
    }

    #[test]
    fn test_detect_db_credentials_postgres_full() {
        let mut env = std::collections::HashMap::new();
        env.insert("POSTGRES_USER".to_string(), "pguser".to_string());
        env.insert("POSTGRES_PASSWORD".to_string(), "pgpass".to_string());
        env.insert("POSTGRES_DB".to_string(), "pgdb".to_string());
        env.insert("PGPORT".to_string(), "5433".to_string());
        let creds = detect_db_credentials(&DatabaseType::PostgreSQL, &env, None);
        assert_eq!(creds.username.as_deref(), Some("pguser"));
        assert_eq!(creds.password.as_deref(), Some("pgpass"));
        assert_eq!(creds.database_name.as_deref(), Some("pgdb"));
        assert_eq!(creds.port.as_deref(), Some("5433"));
    }

    #[test]
    fn test_detect_db_credentials_mysql_full() {
        let mut env = std::collections::HashMap::new();
        env.insert("MYSQL_USER".to_string(), "myuser".to_string());
        env.insert("MYSQL_ROOT_PASSWORD".to_string(), "rootpw".to_string());
        env.insert("MYSQL_DATABASE".to_string(), "mydb".to_string());
        let creds = detect_db_credentials(&DatabaseType::MySQL, &env, None);
        assert_eq!(creds.username.as_deref(), Some("myuser"));
        assert_eq!(creds.password.as_deref(), Some("rootpw"));
        assert_eq!(creds.database_name.as_deref(), Some("mydb"));
    }
}
