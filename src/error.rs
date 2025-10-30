//! Error handling for Mortimer
//!
//! This module defines the error types used throughout the application,
//! providing clear error messages and proper error propagation.

use std::io;
use std::path::PathBuf;
use thiserror::Error;

/// Result type alias for Mortimer operations
pub type Result<T> = std::result::Result<T, Error>;

/// Main error type for Mortimer operations
#[derive(Error, Debug)]
pub enum Error {
    /// IO operation failed
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    /// Regex compilation or execution failed
    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),

    /// JSON serialization/deserialization failed
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Database operation failed
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    /// Configuration file not found
    #[error("Configuration file not found: {path}")]
    ConfigNotFound { path: PathBuf },

    /// History file not found
    #[error("History file not found: {path}")]
    HistoryFileNotFound { path: PathBuf },

    /// Invalid history file format
    #[error("Invalid history file format in {path}: {reason}")]
    InvalidHistoryFormat { path: PathBuf, reason: String },

    /// Home directory could not be determined
    #[error("Home directory not found")]
    HomeDirectoryNotFound,

    /// Invalid command line arguments
    #[error("Invalid arguments: {message}")]
    InvalidArguments { message: String },

    /// Command not found in history
    #[error("Command not found in history")]
    CommandNotFound,

    /// Invalid timestamp format
    #[error("Invalid timestamp format: {timestamp}")]
    InvalidTimestamp { timestamp: String },

    /// Permission denied
    #[error("Permission denied: {path}")]
    PermissionDenied { path: PathBuf },

    /// File already exists
    #[error("File already exists: {path}")]
    FileExists { path: PathBuf },

    /// Invalid redaction pattern
    #[error("Invalid redaction pattern: {pattern}")]
    InvalidRedactionPattern { pattern: String },

    /// Shell integration error
    #[error("Shell integration error: {shell} - {reason}")]
    ShellIntegration { shell: String, reason: String },

    /// Import operation failed
    #[error("Import failed from {from}: {reason}")]
    ImportFailed { from: String, reason: String },

    /// Search operation failed
    #[error("Search failed: {reason}")]
    SearchFailed { reason: String },

    /// Configuration validation failed
    #[error("Configuration validation failed: {field} - {reason}")]
    ConfigValidation { field: String, reason: String },

    /// Generic error with custom message
    #[error("{message}")]
    Custom { message: String },
}

impl Error {
    /// Create a custom error with a message
    pub fn custom<S: Into<String>>(message: S) -> Self {
        Error::Custom {
            message: message.into(),
        }
    }

    /// Create an invalid arguments error
    pub fn invalid_arguments<S: Into<String>>(message: S) -> Self {
        Error::InvalidArguments {
            message: message.into(),
        }
    }

    /// Create a config validation error
    pub fn config_validation<S: Into<String>>(field: S, reason: S) -> Self {
        Error::ConfigValidation {
            field: field.into(),
            reason: reason.into(),
        }
    }

    /// Create an import failed error
    pub fn import_failed<S: Into<String>>(from: S, reason: S) -> Self {
        Error::ImportFailed {
            from: from.into(),
            reason: reason.into(),
        }
    }

    /// Create a search failed error
    pub fn search_failed<S: Into<String>>(reason: S) -> Self {
        Error::SearchFailed {
            reason: reason.into(),
        }
    }

    /// Check if this error is recoverable
    pub fn is_recoverable(&self) -> bool {
        match self {
            Error::Io(_) => true,
            Error::ConfigNotFound { .. } => true,
            Error::HistoryFileNotFound { .. } => true,
            Error::CommandNotFound => true,
            Error::InvalidArguments { .. } => false,
            Error::PermissionDenied { .. } => false,
            Error::HomeDirectoryNotFound => false,
            _ => true,
        }
    }

    /// Get the error category for logging purposes
    pub fn category(&self) -> &'static str {
        match self {
            Error::Io(_) => "io",
            Error::Regex(_) => "regex",
            Error::Json(_) => "json",
            Error::Database(_) => "database",
            Error::ConfigNotFound { .. } | Error::ConfigValidation { .. } => "config",
            Error::HistoryFileNotFound { .. } | Error::InvalidHistoryFormat { .. } => "history",
            Error::HomeDirectoryNotFound => "system",
            Error::InvalidArguments { .. } => "arguments",
            Error::CommandNotFound => "search",
            Error::InvalidTimestamp { .. } => "timestamp",
            Error::PermissionDenied { .. } => "permission",
            Error::FileExists { .. } => "file",
            Error::InvalidRedactionPattern { .. } => "redaction",
            Error::ShellIntegration { .. } => "shell",
            Error::ImportFailed { .. } => "import",
            Error::SearchFailed { .. } => "search",
            Error::Custom { .. } => "custom",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_custom_error() {
        let err = Error::custom("test message");
        assert_eq!(err.to_string(), "test message");
        assert_eq!(err.category(), "custom");
    }

    #[test]
    fn test_invalid_arguments_error() {
        let err = Error::invalid_arguments("missing required argument");
        assert_eq!(
            err.to_string(),
            "Invalid arguments: missing required argument"
        );
        assert_eq!(err.category(), "arguments");
        assert!(!err.is_recoverable());
    }

    #[test]
    fn test_config_validation_error() {
        let err = Error::config_validation("max_entries", "must be positive");
        assert_eq!(
            err.to_string(),
            "Configuration validation failed: max_entries - must be positive"
        );
        assert_eq!(err.category(), "config");
    }

    #[test]
    fn test_history_file_not_found() {
        let path = Path::new("/nonexistent/history").to_path_buf();
        let err = Error::HistoryFileNotFound { path: path.clone() };
        assert_eq!(
            err.to_string(),
            format!("History file not found: {}", path.display())
        );
        assert_eq!(err.category(), "history");
        assert!(err.is_recoverable());
    }

    #[test]
    fn test_permission_denied() {
        let path = Path::new("/root/protected").to_path_buf();
        let err = Error::PermissionDenied { path: path.clone() };
        assert_eq!(
            err.to_string(),
            format!("Permission denied: {}", path.display())
        );
        assert_eq!(err.category(), "permission");
        assert!(!err.is_recoverable());
    }

    #[test]
    fn test_import_failed() {
        let err = Error::import_failed("zsh", "invalid format");
        assert_eq!(err.to_string(), "Import failed from zsh: invalid format");
        assert_eq!(err.category(), "import");
    }

    #[test]
    fn test_search_failed() {
        let err = Error::search_failed("no matches found");
        assert_eq!(err.to_string(), "Search failed: no matches found");
        assert_eq!(err.category(), "search");
    }

    #[test]
    fn test_error_recovery() {
        let recoverable = Error::CommandNotFound;
        assert!(recoverable.is_recoverable());

        let non_recoverable = Error::HomeDirectoryNotFound;
        assert!(!non_recoverable.is_recoverable());
    }
}
