// ── Unified Migration Types ───────────────────────────────────────
// Wave 1 (ramupel): Single data model for both container and compose
// migration paths. Replaces the dual type hierarchy with one unified
// MigrationPlan, UnifiedVolume, UnifiedDatabase, UnifiedEnvVar,
// UnifiedService, PreflightResults, etc.
//
// All types use #[serde(rename_all = "camelCase")] for frontend
// JavaScript convention compatibility.

use serde::{Deserialize, Serialize};

// Re-use DatabaseType from compose_diff.rs (single source of truth).
use crate::compose_diff::DatabaseType;

// ── Core Plan ────────────────────────────────────────────────────

/// The universal migration plan — produced by both container and compose discovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationPlan {
    pub plan_id: String,
    pub migration_type: MigrationType,
    pub source_endpoint: String,
    pub target_endpoint: String,
    pub stack_name: String,
    pub target_stack_name: Option<String>,
    pub source_architecture: Option<String>,
    pub target_architecture: Option<String>,

    pub volumes: Vec<UnifiedVolume>,
    pub databases: Vec<UnifiedDatabase>,
    pub env_vars: Vec<UnifiedEnvVar>,
    pub services: Vec<UnifiedService>,
    pub images: Vec<ImageOverride>,

    pub warnings: Vec<String>,
    pub estimated_size_bytes: u64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum MigrationType {
    Container,
    Compose,
}

// ── Unified Volume ───────────────────────────────────────────────

/// Single volume type replacing both MigrationVolume and VolumeChange.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnifiedVolume {
    pub source_name: String,
    pub target_name: String,
    pub driver: Option<String>,
    pub target_driver: Option<String>,
    pub size_bytes: Option<u64>,
    pub mount_point: Option<String>,
    pub skip: bool,
    pub transfer_method: Option<String>,
}

// ── Unified Database ─────────────────────────────────────────────

/// Single DB type replacing DatabaseService and DbConnection.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnifiedDatabase {
    pub service_name: String,
    pub db_type: DatabaseType,
    pub username: Option<String>,
    pub password: Option<String>,
    pub password_masked: Option<String>,
    pub port: Option<String>,
    pub database_name: Option<String>,
    pub image: String,
    pub version: Option<String>,
    pub pre_transfer_commands: Vec<String>,
    pub post_transfer_commands: Vec<String>,
    pub has_replication: bool,
    pub connectivity_verified: bool,
}

// ── Unified Env Var ──────────────────────────────────────────────

/// Single env var type for both paths.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnifiedEnvVar {
    pub service_name: String,
    pub var_name: String,
    pub source_value: Option<String>,
    pub target_value: Option<String>,
    pub is_sensitive: bool,
    pub will_break: bool,
    pub break_reason: Option<String>,
}

// ── Unified Service ──────────────────────────────────────────────

/// Service-level overrides (mainly for compose path).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnifiedService {
    pub name: String,
    pub action: ServiceAction,
    pub image_override: Option<String>,
    pub restart_policy: Option<String>,
    pub port_overrides: Vec<PortMapping>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ServiceAction {
    Migrate,
    Skip,
    AddTargetOnly,
}

// ── Image Override ───────────────────────────────────────────────

/// Image override (maps to ImageChange from compose diff).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageOverride {
    pub service_name: String,
    pub old_image: String,
    pub new_image: String,
    pub major_version_change: bool,
}

// ── Port Mapping ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortMapping {
    pub container_port: u16,
    pub host_port: Option<u16>,
    pub protocol: Option<String>,
}

// ── Pre-flight ───────────────────────────────────────────────────

/// Results of pre-flight checks.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreflightResults {
    pub checks: Vec<CheckResult>,
}

/// A single check result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckResult {
    pub name: String,
    /// "pass", "warn", or "fail"
    pub status: String,
    pub message: String,
    pub suggestion: Option<String>,
}

// ── Edit Request ─────────────────────────────────────────────────

/// Partial edit payload for updating a stored MigrationPlan.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanEditRequest {
    #[serde(default)]
    pub target_stack_name: Option<String>,
    #[serde(default)]
    pub volumes: Option<Vec<UnifiedVolume>>,
    #[serde(default)]
    pub databases: Option<Vec<UnifiedDatabase>>,
    #[serde(default)]
    pub env_vars: Option<Vec<UnifiedEnvVar>>,
    #[serde(default)]
    pub services: Option<Vec<UnifiedService>>,
    #[serde(default)]
    pub images: Option<Vec<ImageOverride>>,
}

// ── Analyzed Request ─────────────────────────────────────────────

/// Request body for POST /api/migration/unified/analyze
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnifiedAnalyzeRequest {
    pub migration_type: MigrationType,
    pub source_endpoint: String,
    pub target_endpoint: String,
    #[serde(default)]
    pub stack_name: Option<String>,
    #[serde(default)]
    pub container_name: Option<String>,
}
