//! Tunnel-loom relay protocol — shared message types and validation.
//! Zero I/O, zero crypto, zero async. Pure data types.
//!
//! This crate defines the contract between the relay agent and Marionette's
//! controller bridge. Both binaries depend on this crate, ensuring type-safe,
//! compile-time-checked protocol conformance.
//!
//! ## Modules
//! - `message` — Wire message envelope and type discriminators
//! - `operations` — All 30 operation codes with metadata
//! - `errors` — All 31 error codes in dot-namespaced format
//! - `payloads` — Type-safe request/response payload types
//! - `validate` — Message validation (size, schema, operation)

pub mod message;
pub mod operations;
pub mod errors;
pub mod validate;
pub mod payloads;

pub use message::{Message, MessageType, MAX_MESSAGE_SIZE};
pub use operations::OperationCode;
pub use errors::{ErrorCode, ErrorPayload};
