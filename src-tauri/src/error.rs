//! Unified error type for the entire backend.
//!
//! Every fallible operation in this crate returns `Result<T, ForageError>`.
//! The error converts cleanly into a string that the Tauri frontend can show
//! to the user — we never leak file paths or internal detail beyond what is
//! useful for debugging.

use serde::{Serialize, Serializer};
use thiserror::Error;

/// Top-level error type. `thiserror` generates both `Display` and `Error`.
#[derive(Debug, Error)]
pub enum ForageError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Service '{service}' error: {message}")]
    Service { service: String, message: String },

    #[error("Port {port} is already in use by another process")]
    PortInUse { port: u16 },

    #[error("Binary not found: {name}. Tried: {locations}")]
    BinaryNotFound { name: String, locations: String },

    #[error("Download failed: {0}")]
    Download(String),

    #[error("Checksum mismatch for {file}: expected {expected}, got {actual}")]
    ChecksumMismatch {
        file: String,
        expected: String,
        actual: String,
    },

    #[error("Database error: {0}")]
    Database(String),

    #[error("Project error: {0}")]
    Project(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Not initialized — run the first-run wizard first")]
    NotInitialized,

    #[error("Already running: {0}")]
    AlreadyRunning(String),

    #[error("Not running: {0}")]
    NotRunning(String),

    #[error("Tauri error: {0}")]
    Tauri(#[from] tauri::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Serialize as a plain string so the JS side gets a readable error.
impl Serialize for ForageError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

/// Convenience alias used everywhere in the crate.
pub type ForageResult<T> = Result<T, ForageError>;

impl ForageError {
    /// Stable error code the frontend can switch on for localized messages.
    pub fn code(&self) -> &'static str {
        match self {
            ForageError::Io(_) => "io",
            ForageError::Config(_) => "config",
            ForageError::Service { .. } => "service",
            ForageError::PortInUse { .. } => "port_in_use",
            ForageError::BinaryNotFound { .. } => "binary_not_found",
            ForageError::Download(_) => "download",
            ForageError::ChecksumMismatch { .. } => "checksum_mismatch",
            ForageError::Database(_) => "database",
            ForageError::Project(_) => "project",
            ForageError::InvalidInput(_) => "invalid_input",
            ForageError::NotInitialized => "not_initialized",
            ForageError::AlreadyRunning(_) => "already_running",
            ForageError::NotRunning(_) => "not_running",
            ForageError::Tauri(_) => "tauri",
            ForageError::Other(_) => "other",
        }
    }
}
