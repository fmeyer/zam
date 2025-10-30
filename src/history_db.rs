//! Database-backed history management for Mortimer
//!
//! This module provides SQLite-based history management with support for:
//! - Multi-host history tracking
//! - Session management
//! - Token extraction and storage
//! - Migration from legacy formats

use crate::config::Config;
use crate::database::{CommandEntry, Database, DatabaseStats};
use crate::error::{Error, Result};
use crate::redaction::RedactionEngine;
use chrono::{DateTime, Utc};
use regex::Regex;
use std::env;
use std::path::{Path, PathBuf};

/// Database-backed history manager
pub struct HistoryManagerDb {
    config: Config,
    db: Database,
    redaction_engine: RedactionEngine,
}

/// Represents a redacted token extracted from a command
#[derive(Debug, Clone)]
pub struct ExtractedToken {
    pub token_type: String,
    pub placeholder: String,
    pub original_value: String,
}

impl HistoryManagerDb {
    /// Create a new database-backed history manager
    pub fn new(config: Config) -> Result<Self> {
        let redaction_engine = RedactionEngine::with_config(
            config.redaction.use_builtin_patterns,
            config.redaction.custom_patterns.clone(),
            config.redaction.exclude_patterns.clone(),
            config.redaction.placeholder.clone(),
            config.redaction.min_redaction_length,
            config.custom_env_vars.clone(),
            config.redaction.redact_env_vars,
        )?;

        // Get database path from config or default
        let db_path = config.history_file.with_extension("db");

        let db = Database::new(&db_path)?;

        Ok(Self {
            config,
            db,
            redaction_engine,
        })
    }

    /// Log a command to the database
    pub fn log_command(&mut self, command: &str) -> Result<()> {
        self.log_command_with_timestamp(command, None, None)
    }

    /// Log a command with a specific timestamp and exit code
    pub fn log_command_with_timestamp(
        &mut self,
        command: &str,
        timestamp: Option<DateTime<Utc>>,
        exit_code: Option<i32>,
    ) -> Result<()> {
        // Check if we should exclude this command
        if self.config.should_exclude_command(command) {
            return Ok(());
        }

        let timestamp = timestamp.unwrap_or_else(|| Utc::now());
        let directory = env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("<unknown>"))
            .to_string_lossy()
            .to_string();

        // Redact sensitive information and extract tokens
        let (redacted_command, tokens) = if self.config.enable_redaction {
            let (redacted, extracted) = self.redact_and_extract_tokens(command)?;
            let was_redacted = redacted != command;
            (redacted, if was_redacted { extracted } else { vec![] })
        } else {
            (command.to_string(), vec![])
        };

        // Add command to database
        let command_id = self.db.add_command(
            &redacted_command,
            &directory,
            timestamp,
            !tokens.is_empty(),
            exit_code,
        )?;

        // Store extracted tokens
        for token in tokens {
            self.db.store_token(
                command_id,
                &token.token_type,
                &token.placeholder,
                &token.original_value,
            )?;
        }

        Ok(())
    }

    /// Redact a command and extract tokens for storage
    fn redact_and_extract_tokens(&self, command: &str) -> Result<(String, Vec<ExtractedToken>)> {
        let mut tokens = Vec::new();
        let mut redacted = command.to_string();

        // Define patterns for token extraction
        let patterns = vec![
            (
                r#"(?i)(?:password|passwd|pwd)[\s=:]+['"]?([^\s'"]{3,})['"]?"#,
                "password",
            ),
            (
                r#"(?i)(?:token|api_key|apikey|api-key)[\s=:]+['"]?([^\s'"]{10,})['"]?"#,
                "api_key",
            ),
            (
                r#"(?i)(?:secret|secret_key|secretkey)[\s=:]+['"]?([^\s'"]{10,})['"]?"#,
                "secret",
            ),
            (
                r#"(?i)(?:bearer|authorization)[\s:]+['"]?([^\s'"]{10,})['"]?"#,
                "bearer_token",
            ),
            (r#"(?i)--password[=\s]+['"]?([^\s'"]{3,})['"]?"#, "password"),
            (r#"(?i)-p\s+['"]?([^\s'"]{3,})['"]?"#, "password"),
        ];

        for (pattern_str, token_type) in patterns {
            let re = Regex::new(pattern_str)?;

            for caps in re.captures_iter(&redacted.clone()) {
                if let Some(matched) = caps.get(1) {
                    let original_value = matched.as_str().to_string();

                    // Skip if too short (likely not a real password)
                    if original_value.len() < self.config.redaction.min_redaction_length {
                        continue;
                    }

                    // Create placeholder
                    let placeholder = format!("<{}:{}>", token_type, tokens.len() + 1);

                    // Replace in redacted command
                    redacted = redacted.replace(&original_value, &placeholder);

                    tokens.push(ExtractedToken {
                        token_type: token_type.to_string(),
                        placeholder,
                        original_value,
                    });
                }
            }
        }

        // Also apply standard redaction engine
        if tokens.is_empty() {
            redacted = self.redaction_engine.redact(&redacted)?;
        }

        Ok((redacted, tokens))
    }

    /// Search commands in the database
    pub fn search(
        &self,
        query: &str,
        directory_filter: Option<&str>,
        host_filter: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<CommandEntry>> {
        self.db
            .search_commands(query, directory_filter, host_filter, limit)
    }

    /// Get recent commands
    pub fn get_recent(&self, limit: usize) -> Result<Vec<CommandEntry>> {
        self.db.get_recent_commands(limit)
    }

    /// Get all commands (for export)
    pub fn get_all_commands(&self) -> Result<Vec<CommandEntry>> {
        self.db.get_all_commands()
    }

    /// Get database statistics
    pub fn get_stats(&self) -> Result<DatabaseStats> {
        self.db.get_stats()
    }

    /// Get tokens for a specific command
    pub fn get_tokens_for_command(&self, command_id: i64) -> Result<Vec<crate::database::Token>> {
        self.db.get_tokens_for_command(command_id)
    }

    /// Get tokens by session ID
    pub fn get_tokens_by_session(&self, session_id: &str) -> Result<Vec<crate::database::Token>> {
        self.db.get_tokens_by_session(session_id)
    }

    /// Get tokens by directory
    pub fn get_tokens_by_directory(&self, directory: &str) -> Result<Vec<crate::database::Token>> {
        self.db.get_tokens_by_directory(directory)
    }

    /// Start a new session
    pub fn start_session(&mut self) -> Result<String> {
        self.db.start_session()
    }

    /// End a session
    pub fn end_session(&mut self, session_id: &str) -> Result<()> {
        self.db.end_session(session_id)
    }

    /// Import from legacy .mhist file
    pub fn import_from_mhist(&mut self, path: &Path) -> Result<usize> {
        if !path.exists() {
            return Err(Error::HistoryFileNotFound {
                path: path.to_path_buf(),
            });
        }

        self.db.import_from_mhist(path)
    }

    /// Import from bash history
    pub fn import_from_bash(&mut self, path: Option<PathBuf>) -> Result<usize> {
        let history_path = if let Some(p) = path {
            p
        } else {
            // Try to find bash history in default location
            let home = home::home_dir().ok_or(Error::HomeDirectoryNotFound)?;
            home.join(".bash_history")
        };

        if !history_path.exists() {
            return Err(Error::HistoryFileNotFound { path: history_path });
        }

        self.db.import_from_bash_history(&history_path)
    }

    /// Import from zsh history
    pub fn import_from_zsh(&mut self, path: Option<PathBuf>) -> Result<usize> {
        let history_path = if let Some(p) = path {
            p
        } else {
            // Try to find zsh history in default location
            let home = home::home_dir().ok_or(Error::HomeDirectoryNotFound)?;
            let zdotdir = env::var("ZDOTDIR").ok().map(PathBuf::from);
            let base_dir = zdotdir.unwrap_or(home);
            base_dir.join(".zsh_history")
        };

        if !history_path.exists() {
            return Err(Error::HistoryFileNotFound { path: history_path });
        }

        self.db.import_from_zsh_history(&history_path)
    }

    /// Import from fish history
    pub fn import_from_fish(&mut self, path: Option<PathBuf>) -> Result<usize> {
        let history_path = if let Some(p) = path {
            p
        } else {
            // Try to find fish history in default location
            let home = home::home_dir().ok_or(Error::HomeDirectoryNotFound)?;
            let config_dir = env::var("XDG_CONFIG_HOME")
                .ok()
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".config"));
            config_dir.join("fish").join("fish_history")
        };

        if !history_path.exists() {
            return Err(Error::HistoryFileNotFound { path: history_path });
        }

        // Fish history format is YAML-like, we'll do basic parsing
        let content = std::fs::read_to_string(&history_path)?;
        let mut imported_count = 0;

        let mut current_cmd: Option<String> = None;
        let mut current_time: Option<DateTime<Utc>> = None;

        for line in content.lines() {
            let line = line.trim();

            if line.starts_with("- cmd: ") {
                // Save previous command if exists
                if let (Some(cmd), Some(time)) = (current_cmd.take(), current_time.take()) {
                    self.db.add_command(&cmd, "<imported>", time, false, None)?;
                    imported_count += 1;
                }

                current_cmd = Some(line.trim_start_matches("- cmd: ").to_string());
            } else if line.starts_with("when: ") {
                if let Ok(timestamp) = line.trim_start_matches("when: ").parse::<i64>() {
                    if let Some(dt) = DateTime::from_timestamp(timestamp, 0) {
                        current_time = Some(dt);
                    }
                }
            }
        }

        // Don't forget the last command
        if let (Some(cmd), Some(time)) = (current_cmd, current_time) {
            self.db.add_command(&cmd, "<imported>", time, false, None)?;
            imported_count += 1;
        }

        Ok(imported_count)
    }

    /// Merge from another database file
    pub fn merge_from_database(&mut self, other_db_path: &Path) -> Result<usize> {
        if !other_db_path.exists() {
            return Err(Error::HistoryFileNotFound {
                path: other_db_path.to_path_buf(),
            });
        }

        self.db.merge_from_database(other_db_path)
    }

    /// Get all hosts in the database
    pub fn get_hosts(&self) -> Result<Vec<crate::database::Host>> {
        self.db.get_hosts()
    }

    /// Get sessions for a host
    pub fn get_sessions_for_host(&self, host_id: i64) -> Result<Vec<crate::database::Session>> {
        self.db.get_sessions_for_host(host_id)
    }

    /// Clear all data (use with caution!)
    pub fn clear(&self) -> Result<()> {
        self.db.clear()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::{NamedTempFile, TempDir};

    fn test_config() -> (Config, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let temp_file = temp_dir.path().join("test.mhist");
        let mut config = Config::default();
        config.history_file = temp_file;
        config.enable_redaction = true;
        config.shell_integration.exclude_commands.clear();
        (config, temp_dir)
    }

    #[test]
    fn test_history_manager_creation() {
        let (config, _temp_dir) = test_config();
        let manager = HistoryManagerDb::new(config);
        assert!(manager.is_ok());
    }

    #[test]
    fn test_log_command() {
        let (config, _temp_dir) = test_config();
        let mut manager = HistoryManagerDb::new(config).unwrap();

        let result = manager.log_command("ls -la");
        assert!(result.is_ok());

        let stats = manager.get_stats().unwrap();
        assert_eq!(stats.total_commands, 1);
    }

    #[test]
    fn test_redaction_with_tokens() {
        let (config, _temp_dir) = test_config();
        let mut manager = HistoryManagerDb::new(config).unwrap();

        manager
            .log_command("mysql -u root -p secret123 -h localhost")
            .unwrap();

        let commands = manager.get_recent(10).unwrap();
        assert_eq!(commands.len(), 1);
        assert!(commands[0].redacted);
        assert!(!commands[0].command.contains("secret123"));
    }

    #[test]
    fn test_token_extraction() {
        let (config, _temp_dir) = test_config();
        let manager = HistoryManagerDb::new(config).unwrap();

        let (redacted, tokens) = manager
            .redact_and_extract_tokens("export API_KEY=abc123xyz456")
            .unwrap();

        assert!(!tokens.is_empty());
        assert!(!redacted.contains("abc123xyz456"));
    }

    #[test]
    fn test_search() {
        let (config, _temp_dir) = test_config();
        let mut manager = HistoryManagerDb::new(config).unwrap();

        manager.log_command("git status").unwrap();
        manager.log_command("git commit -m 'test'").unwrap();
        manager.log_command("ls -la").unwrap();

        let results = manager.search("git", None, None, None).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_token_retrieval() {
        let (config, _temp_dir) = test_config();
        let mut manager = HistoryManagerDb::new(config).unwrap();

        manager.log_command("export PASSWORD=mypass123").unwrap();

        let commands = manager.get_recent(1).unwrap();
        let tokens = manager.get_tokens_for_command(commands[0].id).unwrap();

        assert!(!tokens.is_empty());
    }

    #[test]
    fn test_multiline_mhist_import() {
        use std::io::Write;
        let (config, _temp_dir) = test_config();
        let mut manager = HistoryManagerDb::new(config).unwrap();

        let mut temp_mhist = NamedTempFile::new().unwrap();
        writeln!(temp_mhist, "2025-10-27 19:39:35 | /tmp | echo 'line1'").unwrap();
        writeln!(
            temp_mhist,
            "2025-10-27 19:40:00 | /tmp | fio --name=test \\"
        )
        .unwrap();
        writeln!(temp_mhist, "    --size=1G \\").unwrap();
        writeln!(temp_mhist, "    --direct=1").unwrap();
        writeln!(temp_mhist, "2025-10-27 19:41:00 | /tmp | ls").unwrap();
        temp_mhist.flush().unwrap();

        let count = manager.import_from_mhist(temp_mhist.path()).unwrap();
        assert_eq!(count, 3);

        let commands = manager.get_all_commands().unwrap();
        assert!(commands[1].command.contains("fio"));
        assert!(commands[1].command.contains("--size=1G"));
        assert!(commands[1].command.contains("--direct=1"));
    }
}
