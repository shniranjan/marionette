//! Migration pipeline — Phase 8 per tunnel-loom spec §9.
//!
//! ## Architecture
//!
//! The migration pipeline is a relay-only orchestration layer. Every
//! remote operation flows through `crate::ws_relay::send_relay_command()`.
//! Nothing calls bollard directly for remote hosts.
//!
//! ## State machine
//!
//! `MigrationStateMachine` enforces a compile-time-validated state graph:
//!
//! ```text
//! Idle → Analyzing → Analyzed → Preparing → Prepared → Transferring
//!   → Transferred → Switching → Complete → Idle
//!                            ↘ RollingBack → RolledBack → Idle
//!                            ↘ Failed → Idle
//! ```
//!
//! Every phase produces a typed result and streams progress via an
//! `event_tx` channel for the frontend WebSocket.
//!
//! ## Submodules
//!
//! - `state` — `MigrationState` enum, `MigrationStateMachine`, checkpoint
//! - `analyze` — `analyze_migration()`, source/target inspection, compose diff
//! - `prepare` — target provisioning (Wave 2)
//! - `transfer` — volume transfer engine (Wave 2)
//! - `switchover` — source stop / target deploy / health check (Wave 3)
//! - `rollback` — reverse switchover (Wave 3)

pub mod analyze;
pub mod state;

// Wave 2 placeholders — declared so route handlers can call stubs.
pub mod prepare;
pub mod transfer;

// Wave 3 modules
pub mod switchover;
pub mod rollback;

// ── Re-exports ───────────────────────────────────────────────────────

pub use analyze::{
    analyze_migration, AnalyzeResult, BindMount, ComposeDiff, DbConnection, EnvChange,
    HostSummary, ServicePlan, VolumePlan,
};
pub use state::{MigrationCheckpoint, MigrationState, MigrationStateMachine};

pub use prepare::{prepare_migration, ImageResult, PrepareResult};
pub use transfer::{execute_transfer, TransferResult, VolumeTransferOutcome};
pub use switchover::{execute_switchover, SwitchoverResult};
pub use rollback::{rollback_migration, RollbackResult};

// ── Event types for frontend streaming ───────────────────────────────

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::LazyLock;
use tokio::sync::Mutex;

/// In-memory plan storage keyed by plan_id.
/// Populated by the analyze handler, read by all downstream handlers.
pub static PLANS: LazyLock<Mutex<HashMap<String, AnalyzeResult>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// In-memory migration state keyed by plan_id.
pub static MIGRATION_STATES: LazyLock<Mutex<HashMap<String, MigrationState>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// In-memory event log keyed by plan_id.
pub static MIGRATION_EVENTS: LazyLock<Mutex<HashMap<String, Vec<MigrationEvent>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Events emitted during migration pipeline execution and streamed to
/// the frontend WebSocket.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum MigrationEvent {
    /// Phase transition (e.g., Idle → Analyzing).
    PhaseChange {
        from: MigrationState,
        to: MigrationState,
    },
    /// Progress within a phase (percent, message, etc.).
    PhaseProgress {
        phase: String,
        percent: f64,
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        details: Option<serde_json::Value>,
    },
    /// Per-volume transfer progress with transfer metrics.
    TransferProgress {
        volume: String,
        transfer_id: String,
        bytes_sent: u64,
        total_bytes: u64,
        percent: f64,
        #[serde(skip_serializing_if = "Option::is_none")]
        throughput_mbps: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        eta_secs: Option<f64>,
    },
    /// A non-fatal warning from any phase.
    Warning {
        phase: String,
        message: String,
    },
    /// A fatal error — pipeline stops.
    Error {
        phase: String,
        message: String,
    },
    /// Migration pipeline has completed (success or failure).
    Complete {
        success: bool,
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        summary: Option<serde_json::Value>,
    },
}

// ── Request/response types for route handlers ────────────────────────

/// Request body for POST /api/migration/analyze.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzeMigrationRequest {
    pub source_host: String,
    pub target_host: String,
    pub stack_name: String,
}

/// Request body for POST /api/migration/{prepare,transfer,switchover,rollback}.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteMigrationRequest {
    pub plan_id: String,
    #[serde(default)]
    pub user_overrides: Option<serde_json::Value>,
}

// ── Pipeline orchestrator (stub — body in later waves) ───────────────

use tokio::sync::mpsc;

/// Entry-point for the full migration pipeline.
///
/// Steps through Analyze → Prepare → Transfer → Switchover, emitting
/// progress events to `event_tx` for frontend streaming.
///
/// # Arguments
/// - `source_host`: relay hostname of the source Docker host
/// - `target_host`: relay hostname of the target Docker host
/// - `stack_name`: compose project name / container name
/// - `event_tx`: channel for progress events (frontend WebSocket)
///
/// # Returns
/// `Ok(())` when the pipeline completes successfully, `Err(...)` on
/// unrecoverable failure.
pub async fn run_migration_pipeline(
    _source_host: &str,
    _target_host: &str,
    _stack_name: &str,
    _event_tx: mpsc::UnboundedSender<MigrationEvent>,
) -> Result<(), String> {
    // Stub — body will be implemented in later waves.
    // The full pipeline:
    //   1. Create MigrationStateMachine
    //   2. analyze_migration() → Analyze phase
    //   3. prepare_migration() → Prepare phase
    //   4. execute_transfer() → Transfer phase
    //   5. execute_switchover() → Switchover phase
    //   6. Emit events through event_tx at each step
    tracing::info!("run_migration_pipeline: stub — not yet implemented");
    Ok(())
}
