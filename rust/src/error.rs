use std::io;

use thiserror::Error;

/// Represents errors that can occur when interacting with the SWI-Prolog MQI.
#[derive(Error, Debug)]
pub enum PrologError {
    /// Error related to I/O operations (e.g., socket communication).
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// Error during JSON serialization or deserialization.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Error launching the SWI-Prolog process.
    #[error("Failed to launch SWI-Prolog: {0}")]
    LaunchError(String),

    /// Connection to the MQI server failed or was lost.
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    /// Authentication with the MQI server failed (e.g., wrong password).
    #[error("Authentication failed")]
    AuthenticationFailed,

    /// A specific exception occurred within the Prolog execution.
    #[error("Prolog exception: {kind}{}", term.as_ref().map(|t| format!(" ({})", serde_json::to_string(t).unwrap_or_else(|_| "<unserializable term>".to_string()))).unwrap_or_default())]
    PrologException {
        kind: String,
        term: Option<serde_json::Value>,
    },

    /// The Prolog query timed out.
    #[error("Query timed out")]
    Timeout,

    /// An operation was attempted when no query was active (e.g., cancel_async).
    #[error("No query is currently active")]
    NoQuery,

    /// The active asynchronous query was cancelled.
    #[error("Query was cancelled")]
    QueryCancelled,

    /// The result for an asynchronous query was not available within the specified timeout.
    #[error("Asynchronous result not available yet")]
    ResultNotAvailable,

    /// Protocol version mismatch between the client library and the MQI server.
    #[error("Protocol version mismatch: client requires {client}, server provides {server}")]
    VersionMismatch {
        client: String,
        server: String,
    },

    /// The required feature (e.g., unix-socket) is not enabled.
    #[error("Feature not enabled: {0}")]
    FeatureNotEnabled(String),

    /// Invalid state or configuration.
    #[error("Invalid state: {0}")]
    InvalidState(String),
} 