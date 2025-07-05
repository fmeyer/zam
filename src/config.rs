//! Configuration management for Mortimer
//!
//! This module handles loading, validating, and managing configuration
//! for the Mortimer history manager, including redaction patterns,
//! file paths, and behavior settings.

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Default configuration file name
pub const DEFAULT_CONFIG_FILE: &str = ".mortimer.json";

/// Default maximum number of history entries to keep
pub const DEFAULT_MAX_ENTRIES: usize = 100_000;

/// Default redaction replacement text
pub const DEFAULT_REDACTION_PLACEHOLDER: &str = "<redacted>";

/// Main configuration structure for Mortimer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Path to the history file
    pub history_file: PathBuf,

    /// Maximum number of history entries to keep (0 = unlimited)
    pub max_entries: usize,

    /// Whether to enable automatic redaction of sensitive data
    pub enable_redaction: bool,

    /// Redaction configuration
    pub redaction: RedactionConfig,

    /// Import configuration
    pub import: ImportConfig,

    /// Search configuration
    pub search: SearchConfig,

    /// Logging configuration
    pub logging: LoggingConfig,

    /// Shell integration settings
    pub shell_integration: ShellIntegrationConfig,

    /// Custom environment variables to redact
    pub custom_env_vars: Vec<String>,
}

/// Configuration for redaction behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactionConfig {
    /// Text to replace redacted content with
    pub placeholder: String,

    /// Whether to use built-in redaction patterns
    pub use_builtin_patterns: bool,

    /// Custom redaction patterns (regex)
    pub custom_patterns: Vec<String>,

    /// Patterns to exclude from redaction
    pub exclude_patterns: Vec<String>,

    /// Whether to redact environment variables
    pub redact_env_vars: bool,

    /// Minimum length for values to be considered for redaction
    pub min_redaction_length: usize,
}

/// Configuration for importing history from other shells
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportConfig {
    /// Paths to shell history files
    pub shell_history_paths: HashMap<String, PathBuf>,

    /// Whether to automatically detect shell history files
    pub auto_detect: bool,

    /// Whether to deduplicate entries during import
    pub deduplicate: bool,

    /// Whether to preserve original timestamps
    pub preserve_timestamps: bool,

    /// Maximum age of entries to import (in days, 0 = no limit)
    pub max_age_days: u32,
}

/// Configuration for search functionality
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchConfig {
    /// Whether to enable fuzzy search by default
    pub fuzzy_search: bool,

    /// Whether to enable case-sensitive search by default
    pub case_sensitive: bool,

    /// Whether to include directory information in search results
    pub include_directory: bool,

    /// Whether to include timestamps in search results
    pub include_timestamps: bool,

    /// Maximum number of search results to return
    pub max_results: usize,

    /// Whether to highlight matches in search results
    pub highlight_matches: bool,
}

/// Configuration for logging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level (trace, debug, info, warn, error)
    pub level: String,

    /// Whether to log to file
    pub log_to_file: bool,

    /// Path to log file
    pub log_file: Option<PathBuf>,

    /// Whether to include timestamps in logs
    pub include_timestamps: bool,

    /// Whether to log redacted commands (for debugging)
    pub log_redacted_commands: bool,
}

/// Configuration for shell integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellIntegrationConfig {
    /// Whether to automatically log all commands
    pub auto_log: bool,

    /// Commands to exclude from logging
    pub exclude_commands: Vec<String>,

    /// Whether to log commands that start with a space
    pub log_space_prefixed: bool,

    /// Whether to log duplicate commands
    pub log_duplicates: bool,

    /// Minimum command length to log
    pub min_command_length: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            history_file: crate::default_history_path()
                .unwrap_or_else(|_| PathBuf::from("/tmp").join(crate::DEFAULT_HISTORY_FILE)),
            max_entries: DEFAULT_MAX_ENTRIES,
            enable_redaction: true,
            redaction: RedactionConfig::default(),
            import: ImportConfig::default(),
            search: SearchConfig::default(),
            logging: LoggingConfig::default(),
            shell_integration: ShellIntegrationConfig::default(),
            custom_env_vars: vec![
                "PASSWORD".to_string(),
                "SECRET".to_string(),
                "TOKEN".to_string(),
                "API_KEY".to_string(),
                "PRIVATE_KEY".to_string(),
            ],
        }
    }
}

impl Default for RedactionConfig {
    fn default() -> Self {
        Self {
            placeholder: DEFAULT_REDACTION_PLACEHOLDER.to_string(),
            use_builtin_patterns: true,
            custom_patterns: Vec::new(),
            exclude_patterns: Vec::new(),
            redact_env_vars: true,
            min_redaction_length: 3,
        }
    }
}

impl Default for ImportConfig {
    fn default() -> Self {
        let mut shell_history_paths = HashMap::new();

        // Add default shell history paths
        if let Some(home) = home::home_dir() {
            shell_history_paths.insert("zsh".to_string(), home.join(".histfile"));
            shell_history_paths.insert("bash".to_string(), home.join(".bash_history"));
            shell_history_paths.insert(
                "fish".to_string(),
                home.join(".local/share/fish/fish_history"),
            );
        }

        Self {
            shell_history_paths,
            auto_detect: true,
            deduplicate: true,
            preserve_timestamps: true,
            max_age_days: 0, // No limit
        }
    }
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            fuzzy_search: true,
            case_sensitive: false,
            include_directory: true,
            include_timestamps: false,
            max_results: 1000,
            highlight_matches: true,
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            log_to_file: false,
            log_file: None,
            include_timestamps: true,
            log_redacted_commands: false,
        }
    }
}

impl Default for ShellIntegrationConfig {
    fn default() -> Self {
        Self {
            auto_log: true,
            exclude_commands: vec![
                "ls".to_string(),
                "cd".to_string(),
                "pwd".to_string(),
                "clear".to_string(),
                "history".to_string(),
            ],
            log_space_prefixed: false,
            log_duplicates: false,
            min_command_length: 1,
        }
    }
}

impl Config {
    /// Load configuration from the default location
    pub fn load() -> Result<Self> {
        let config_path = Self::default_config_path()?;
        Self::load_from_path(&config_path)
    }

    /// Load configuration from a specific path
    pub fn load_from_path(path: &PathBuf) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(path).map_err(|e| Error::Io(e))?;

        let config: Config = serde_json::from_str(&content).map_err(|e| Error::Json(e))?;

        config.validate()?;
        Ok(config)
    }

    /// Save configuration to the default location
    pub fn save(&self) -> Result<()> {
        let config_path = Self::default_config_path()?;
        self.save_to_path(&config_path)
    }

    /// Save configuration to a specific path
    pub fn save_to_path(&self, path: &PathBuf) -> Result<()> {
        self.validate()?;

        // Create parent directories if they don't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)?;

        Ok(())
    }

    /// Get the default configuration file path
    pub fn default_config_path() -> Result<PathBuf> {
        let home = home::home_dir().ok_or(Error::HomeDirectoryNotFound)?;
        Ok(home.join(DEFAULT_CONFIG_FILE))
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<()> {
        // Validate redaction patterns
        for pattern in &self.redaction.custom_patterns {
            regex::Regex::new(pattern).map_err(|_| Error::InvalidRedactionPattern {
                pattern: pattern.clone(),
            })?;
        }

        // Validate exclude patterns
        for pattern in &self.redaction.exclude_patterns {
            regex::Regex::new(pattern).map_err(|_| Error::InvalidRedactionPattern {
                pattern: pattern.clone(),
            })?;
        }

        // Validate max entries
        if self.max_entries == 0 {
            return Err(Error::config_validation(
                "max_entries",
                "must be greater than 0 or use a very large number for unlimited",
            ));
        }

        // Validate logging level
        match self.logging.level.as_str() {
            "trace" | "debug" | "info" | "warn" | "error" => {}
            _ => {
                return Err(Error::config_validation(
                    "logging.level",
                    "must be one of: trace, debug, info, warn, error",
                ))
            }
        }

        // Validate search max results
        if self.search.max_results == 0 {
            return Err(Error::config_validation(
                "search.max_results",
                "must be greater than 0",
            ));
        }

        Ok(())
    }

    /// Merge with another configuration, taking values from the other config
    pub fn merge(&mut self, other: &Config) {
        self.history_file = other.history_file.clone();
        self.max_entries = other.max_entries;
        self.enable_redaction = other.enable_redaction;
        self.redaction = other.redaction.clone();
        self.import = other.import.clone();
        self.search = other.search.clone();
        self.logging = other.logging.clone();
        self.shell_integration = other.shell_integration.clone();
        self.custom_env_vars = other.custom_env_vars.clone();
    }

    /// Get all redaction patterns (builtin + custom)
    pub fn get_all_redaction_patterns(&self) -> Vec<String> {
        let mut patterns = Vec::new();

        if self.redaction.use_builtin_patterns {
            // Add builtin patterns - these are defined in the redaction module
            patterns.extend(vec![
                r"(?i)password\s*[=:]\s*[^\s]+".to_string(),
                r"(?i)token\s*[=:]\s*[^\s]+".to_string(),
                r"(?i)secret\s*[=:]\s*[^\s]+".to_string(),
                r"(?i)api_key\s*[=:]\s*[^\s]+".to_string(),
                r"(?i)(://[^:/@]+:)[^@]*(@)".to_string(),
                r"(?i)bearer\s+[a-zA-Z0-9._-]+".to_string(),
            ]);
        }

        patterns.extend(self.redaction.custom_patterns.clone());
        patterns
    }

    /// Check if a command should be excluded from logging
    pub fn should_exclude_command(&self, command: &str) -> bool {
        // Check if command is in exclude list
        for excluded in &self.shell_integration.exclude_commands {
            if command.starts_with(excluded) {
                return true;
            }
        }

        // Check command length
        if command.len() < self.shell_integration.min_command_length {
            return true;
        }

        // Check if command starts with space and we're configured to exclude those
        if !self.shell_integration.log_space_prefixed && command.starts_with(' ') {
            return true;
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(config.enable_redaction);
        assert_eq!(config.max_entries, DEFAULT_MAX_ENTRIES);
        assert_eq!(config.redaction.placeholder, DEFAULT_REDACTION_PLACEHOLDER);
    }

    #[test]
    fn test_config_validation() {
        let mut config = Config::default();
        assert!(config.validate().is_ok());

        // Test invalid regex pattern
        config
            .redaction
            .custom_patterns
            .push("[invalid".to_string());
        assert!(config.validate().is_err());

        // Test invalid max entries
        config.redaction.custom_patterns.clear();
        config.max_entries = 0;
        assert!(config.validate().is_err());

        // Test invalid logging level
        config.max_entries = 1000;
        config.logging.level = "invalid".to_string();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_save_load() {
        let temp_file = NamedTempFile::new().unwrap();
        let config_path = temp_file.path().to_path_buf();

        let mut config = Config::default();
        config.max_entries = 50000;
        config.redaction.placeholder = "<HIDDEN>".to_string();

        // Save configuration
        config.save_to_path(&config_path).unwrap();

        // Load configuration
        let loaded_config = Config::load_from_path(&config_path).unwrap();

        assert_eq!(loaded_config.max_entries, 50000);
        assert_eq!(loaded_config.redaction.placeholder, "<HIDDEN>");
    }

    #[test]
    fn test_should_exclude_command() {
        let config = Config::default();

        // Should exclude commands in the exclude list
        assert!(config.should_exclude_command("ls -la"));
        assert!(config.should_exclude_command("cd /tmp"));

        // Should not exclude other commands
        assert!(!config.should_exclude_command("echo hello"));
        assert!(!config.should_exclude_command("grep pattern file"));
    }

    #[test]
    fn test_get_all_redaction_patterns() {
        let mut config = Config::default();
        config
            .redaction
            .custom_patterns
            .push("custom_pattern".to_string());

        let patterns = config.get_all_redaction_patterns();
        assert!(!patterns.is_empty());
        assert!(patterns.contains(&"custom_pattern".to_string()));
    }

    #[test]
    fn test_config_merge() {
        let mut config1 = Config::default();
        config1.max_entries = 1000;

        let mut config2 = Config::default();
        config2.max_entries = 2000;
        config2.enable_redaction = false;

        config1.merge(&config2);

        assert_eq!(config1.max_entries, 2000);
        assert!(!config1.enable_redaction);
    }
}
