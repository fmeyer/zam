//! Mortimer - Enhanced shell history manager with sensitive data redaction
//!
//! This library provides functionality for:
//! - Logging shell commands with automatic sensitive data redaction
//! - Importing history from various shells (Zsh, Bash)
//! - Searching and filtering command history
//! - Configurable redaction patterns
//!
//! # Examples
//!
//! ```rust
//! use mortimer::{HistoryManager, Config};
//!
//! let config = Config::default();
//! let mut manager = HistoryManager::new(config)?;
//! manager.log_command("echo 'password=secret123'")?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::path::PathBuf;

pub mod cli;
pub mod config;
pub mod error;
pub mod history;
pub mod redaction;
pub mod search;

pub use config::Config;
pub use error::{Error, Result};
pub use history::HistoryManager;
pub use redaction::RedactionEngine;
pub use search::SearchEngine;

/// The default history file name
pub const DEFAULT_HISTORY_FILE: &str = ".mhist";

/// Get the default history file path
pub fn default_history_path() -> Result<PathBuf> {
    let home = home::home_dir().ok_or(Error::HomeDirectoryNotFound)?;
    Ok(home.join(DEFAULT_HISTORY_FILE))
}

/// Initialize the library with default configuration
pub fn init() -> Result<HistoryManager> {
    let config = Config::default();
    HistoryManager::new(config)
}

/// Initialize the library with a custom configuration
pub fn init_with_config(config: Config) -> Result<HistoryManager> {
    HistoryManager::new(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_history_path() {
        let path = default_history_path().unwrap();
        assert!(path.ends_with(DEFAULT_HISTORY_FILE));
    }

    #[test]
    fn test_init() {
        let manager = init();
        assert!(manager.is_ok());
    }
}
