//! Migration state machine — compile-time-validated transitions.
//!
//! The `MigrationStateMachine` enforces the legal state graph. Every
//! transition is checked against `VALID_TRANSITIONS`; invalid moves
//! return an error without mutating state.
//!
//! Checkpoint serialization (JSON) supports pause/resume across restarts.

use serde::{Deserialize, Serialize};
use std::time::Instant;

// ── MigrationState ──────────────────────────────────────────────────

/// All legal states in the migration pipeline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MigrationState {
    Idle,
    Analyzing,
    Analyzed,
    Preparing,
    Prepared,
    Transferring,
    Transferred,
    Switching,
    Complete,
    RollingBack,
    RolledBack,
    Failed,
}

// ── Valid transitions (compile-time array) ──────────────────────────

/// Every `(from, to)` pair that the state machine allows.
/// Transitioning to a state not listed here returns `Err`.
pub const VALID_TRANSITIONS: &[(MigrationState, MigrationState)] = &[
    (MigrationState::Idle, MigrationState::Analyzing),
    (MigrationState::Analyzing, MigrationState::Analyzed),
    (MigrationState::Analyzing, MigrationState::Failed),
    (MigrationState::Analyzed, MigrationState::Preparing),
    (MigrationState::Preparing, MigrationState::Prepared),
    (MigrationState::Preparing, MigrationState::Failed),
    (MigrationState::Prepared, MigrationState::Transferring),
    (MigrationState::Transferring, MigrationState::Transferred),
    (MigrationState::Transferring, MigrationState::Failed),
    (MigrationState::Transferred, MigrationState::Switching),
    (MigrationState::Switching, MigrationState::Complete),
    (MigrationState::Switching, MigrationState::RollingBack),
    (MigrationState::Switching, MigrationState::Failed),
    (MigrationState::RollingBack, MigrationState::RolledBack),
    (MigrationState::RollingBack, MigrationState::Failed),
    (MigrationState::RolledBack, MigrationState::Idle),
    (MigrationState::Complete, MigrationState::Idle),
    (MigrationState::Failed, MigrationState::Idle),
];

// ── MigrationCheckpoint ─────────────────────────────────────────────

/// Persisted snapshot of the state machine for pause/resume.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationCheckpoint {
    pub state: MigrationState,
    pub source_host: String,
    pub target_host: String,
    pub stack_name: String,
    pub completed_volumes: Vec<String>,
    pub timestamp: String,
}

// ── MigrationStateMachine ───────────────────────────────────────────

/// Compile-time-validated state machine for the migration pipeline.
///
/// Every `transition()` call checks the `VALID_TRANSITIONS` table
/// before mutating state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationStateMachine {
    state: MigrationState,
    checkpoint: Option<MigrationCheckpoint>,
    #[serde(skip, default = "Instant::now")]
    started_at: Instant,
    #[serde(skip, default = "Instant::now")]
    last_transition: Instant,
}

impl MigrationStateMachine {
    /// Create a new state machine in the `Idle` state.
    pub fn new() -> Self {
        Self {
            state: MigrationState::Idle,
            checkpoint: None,
            started_at: Instant::now(),
            last_transition: Instant::now(),
        }
    }

    /// Move to `new_state` if `(current, new_state)` is a valid
    /// transition. Returns `Err(...)` on invalid moves.
    pub fn transition(&mut self, new_state: MigrationState) -> Result<(), String> {
        let valid = VALID_TRANSITIONS
            .iter()
            .any(|(from, to)| *from == self.state && *to == new_state);

        if !valid {
            return Err(format!(
                "Invalid state transition: {:?} -> {:?}",
                self.state, new_state
            ));
        }

        self.state = new_state;
        self.last_transition = Instant::now();
        Ok(())
    }

    /// Serialize the current state to a JSON string for disk persistence.
    pub fn checkpoint(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self)
            .map_err(|e| format!("Checkpoint serialize error: {}", e))
    }

    /// Deserialize from a JSON string and resume.
    pub fn resume(json: &str) -> Result<Self, String> {
        let mut machine: Self = serde_json::from_str(json)
            .map_err(|e| format!("Checkpoint deserialize error: {}", e))?;
        // Reset runtime-only fields.
        machine.started_at = Instant::now();
        machine.last_transition = Instant::now();
        Ok(machine)
    }

    /// Returns `true` when the pipeline can still be rolled back.
    pub fn can_rollback(&self) -> bool {
        matches!(self.state, MigrationState::Switching | MigrationState::RollingBack)
    }

    /// Time elapsed since the machine was created (or resumed).
    pub fn elapsed(&self) -> std::time::Duration {
        self.started_at.elapsed()
    }

    /// Time elapsed since the last state transition.
    pub fn phase_duration(&self) -> std::time::Duration {
        self.last_transition.elapsed()
    }

    /// Borrow the current state.
    pub fn state(&self) -> &MigrationState {
        &self.state
    }

    /// Borrow the current checkpoint (if any).
    pub fn checkpoint_data(&self) -> Option<&MigrationCheckpoint> {
        self.checkpoint.as_ref()
    }

    /// Set the checkpoint data (caller builds the struct).
    pub fn set_checkpoint(&mut self, cp: MigrationCheckpoint) {
        self.checkpoint = Some(cp);
    }
}

impl Default for MigrationStateMachine {
    fn default() -> Self {
        Self::new()
    }
}
