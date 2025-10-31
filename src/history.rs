//! History management for Mortimer
//!
//! This module provides comprehensive history management functionality,
//! including logging, importing, searching, and maintaining command history
//! with automatic redaction and deduplication.

use crate::config::Config;
use crate::error::{Error, Result};
use crate::redaction::{RedactionEngine, RedactionStats};
use chrono::{DateTime, Utc};
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::PathBuf;

/// Represents a single command entry in the history
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct HistoryEntry {
    /// The command that was executed
    pub command: String,
    /// Timestamp when the command was executed
    pub timestamp: DateTime<Utc>,
    /// Working directory where the command was executed
    pub directory: String,
    /// Whether this command was redacted
    pub redacted: bool,
    /// Original command before redaction (for debugging, if enabled)
    pub original: Option<String>,
}

/// Statistics about the history
#[derive(Debug, Clone, Default)]
pub struct HistoryStats {
    /// Total number of entries
    pub total_entries: usize,
    /// Number of redacted entries
    pub redacted_entries: usize,
    /// Number of unique commands
    pub unique_commands: usize,
    /// Number of duplicate commands filtered
    pub duplicates_filtered: usize,
    /// Most common directories
    pub common_directories: HashMap<String, usize>,
    /// Redaction statistics
    pub redaction_stats: RedactionStats,
}

/// Main history manager
pub struct HistoryManager {
    config: Config,
    redaction_engine: RedactionEngine,
    history_file: PathBuf,
    stats: HistoryStats,
}

impl HistoryManager {
    /// Create a new history manager with the given configuration
    #[must_use = "History manager must be used to log commands"]
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

        let history_file = config.history_file.clone();

        // Create history file if it doesn't exist
        if !history_file.exists() {
            if let Some(parent) = history_file.parent() {
                std::fs::create_dir_all(parent)?;
            }
            File::create(&history_file)?;
        }

        let mut manager = Self {
            config,
            redaction_engine,
            history_file,
            stats: HistoryStats::default(),
        };

        // Load initial statistics
        manager.update_stats()?;

        Ok(manager)
    }

    /// Log a command to the history
    pub fn log_command(&mut self, command: &str) -> Result<()> {
        self.log_command_with_timestamp(command, None)
    }

    /// Log a command with a specific timestamp
    pub fn log_command_with_timestamp(
        &mut self,
        command: &str,
        timestamp: Option<DateTime<Utc>>,
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

        // Redact sensitive information
        let (redacted_command, was_redacted) = if self.config.enable_redaction {
            let original = command.to_string();
            let redacted = self
                .redaction_engine
                .redact_with_stats(command, &mut self.stats.redaction_stats)?;
            (redacted.clone(), redacted != original)
        } else {
            (command.to_string(), false)
        };

        let entry = HistoryEntry {
            command: redacted_command,
            timestamp,
            directory,
            redacted: was_redacted,
            original: if was_redacted && self.config.logging.log_redacted_commands {
                Some(command.to_string())
            } else {
                None
            },
        };

        // Check for duplicates if configured
        if !self.config.shell_integration.log_duplicates && self.is_duplicate(&entry)? {
            self.stats.duplicates_filtered += 1;
            return Ok(());
        }

        self.write_entry(&entry)?;
        self.update_stats_for_entry(&entry);

        // Trim history if it exceeds max entries
        if self.config.max_entries > 0 && self.stats.total_entries > self.config.max_entries {
            self.trim_history()?;
        }

        Ok(())
    }

    /// Import history from a shell history file
    pub fn import_from_shell(&mut self, shell: &str, file_path: Option<PathBuf>) -> Result<usize> {
        let history_path = if let Some(path) = file_path {
            path
        } else {
            self.config
                .import
                .shell_history_paths
                .get(shell)
                .ok_or_else(|| Error::import_failed(shell, "shell not configured"))?
                .clone()
        };

        if !history_path.exists() {
            return Err(Error::HistoryFileNotFound { path: history_path });
        }

        let file = File::open(&history_path)?;
        let reader = BufReader::new(file);
        let mut imported_count = 0;
        let mut seen_commands = HashSet::new();

        for line in reader.lines() {
            let line = line.unwrap_or_default();
            if line.trim().is_empty() {
                continue;
            }

            let entry = match shell {
                "zsh" => self.parse_zsh_entry(&line)?,
                "bash" => self.parse_bash_entry(&line)?,
                "fish" => self.parse_fish_entry(&line)?,
                _ => return Err(Error::import_failed(shell, "unsupported shell")),
            };

            if let Some(entry) = entry {
                // Check age limit
                if self.config.import.max_age_days > 0 {
                    let age_limit =
                        Utc::now() - chrono::Duration::days(self.config.import.max_age_days as i64);
                    if entry.timestamp < age_limit {
                        continue;
                    }
                }

                // Check for duplicates if deduplication is enabled
                if self.config.import.deduplicate {
                    let key = format!("{}:{}", entry.command, entry.directory);
                    if !seen_commands.insert(key) {
                        continue;
                    }
                }

                self.write_entry(&entry)?;
                imported_count += 1;
            }
        }

        self.update_stats()?;
        Ok(imported_count)
    }

    /// Get all history entries
    #[must_use = "Query results should be used"]
    pub fn get_entries(&self) -> Result<Vec<HistoryEntry>> {
        let file = File::open(&self.history_file)?;
        let reader = BufReader::new(file);
        let mut entries = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if let Some(entry) = self.parse_entry(&line)? {
                entries.push(entry);
            }
        }

        Ok(entries)
    }

    /// Search history entries
    #[must_use = "Search results should be used"]
    pub fn search(&self, query: &str, directory_filter: Option<&str>) -> Result<Vec<HistoryEntry>> {
        let entries = self.get_entries()?;
        let mut results = Vec::new();

        let query_lower = query.to_lowercase();

        for entry in entries {
            // Apply directory filter if specified
            if let Some(dir_filter) = directory_filter {
                if !entry.directory.contains(dir_filter) {
                    continue;
                }
            }

            // Check if command matches query
            let matches = if self.config.search.case_sensitive {
                entry.command.contains(query)
            } else {
                entry.command.to_lowercase().contains(&query_lower)
            };

            if matches {
                results.push(entry);
            }

            // Limit results
            if results.len() >= self.config.search.max_results {
                break;
            }
        }

        Ok(results)
    }

    /// Get unique commands for fuzzy search
    pub fn get_unique_commands(&self) -> Result<Vec<String>> {
        let entries = self.get_entries()?;
        let mut seen = HashSet::new();
        let mut commands = Vec::new();

        // Reverse iteration to get most recent commands first
        for entry in entries.into_iter().rev() {
            if seen.insert(entry.command.clone()) {
                commands.push(entry.command);
            }
        }

        Ok(commands)
    }

    /// Get history statistics
    pub fn get_stats(&self) -> &HistoryStats {
        &self.stats
    }

    /// Clear all history
    pub fn clear(&mut self) -> Result<()> {
        std::fs::write(&self.history_file, "")?;
        self.stats = HistoryStats::default();
        Ok(())
    }

    /// Trim history to max entries
    fn trim_history(&mut self) -> Result<()> {
        let entries = self.get_entries()?;
        let keep_count = self.config.max_entries;

        if entries.len() <= keep_count {
            return Ok(());
        }

        // Keep the most recent entries
        let entries_to_keep = &entries[entries.len() - keep_count..];

        // Rewrite the file with only the entries we want to keep
        let file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&self.history_file)?;
        let mut writer = BufWriter::new(file);

        for entry in entries_to_keep {
            writeln!(writer, "{}", self.format_entry(entry))?;
        }

        writer.flush()?;
        self.update_stats()?;

        Ok(())
    }

    /// Write a single entry to the history file
    fn write_entry(&self, entry: &HistoryEntry) -> Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.history_file)?;

        writeln!(file, "{}", self.format_entry(entry))?;
        Ok(())
    }

    /// Format an entry for writing to file
    fn format_entry(&self, entry: &HistoryEntry) -> String {
        let timestamp_str = entry.timestamp.format("%Y-%m-%d %H:%M:%S").to_string();
        format!(
            "{} | {} | {}",
            timestamp_str, entry.directory, entry.command
        )
    }

    /// Parse a line from the history file
    fn parse_entry(&self, line: &str) -> Result<Option<HistoryEntry>> {
        let parts: Vec<&str> = line.splitn(3, " | ").collect();
        if parts.len() != 3 {
            return Ok(None);
        }

        let timestamp_str = parts[0];
        let directory = parts[1].to_string();
        let command = parts[2].to_string();

        // Parse timestamp
        let timestamp = chrono::NaiveDateTime::parse_from_str(timestamp_str, "%Y-%m-%d %H:%M:%S")
            .map_err(|_| Error::InvalidTimestamp {
                timestamp: timestamp_str.to_string(),
            })?
            .and_utc();

        Ok(Some(HistoryEntry {
            command,
            timestamp,
            directory,
            redacted: false, // We don't store this information in the file
            original: None,
        }))
    }

    /// Parse a Zsh history entry
    fn parse_zsh_entry(&self, line: &str) -> Result<Option<HistoryEntry>> {
        // Zsh format: ": 1609786800:0;command"
        let re = regex::Regex::new(r"^: (\d+):\d+;(.*)").unwrap();

        if let Some(caps) = re.captures(line) {
            let timestamp_str = caps.get(1).unwrap().as_str();
            let command = caps.get(2).unwrap().as_str();

            let timestamp = timestamp_str
                .parse::<i64>()
                .map_err(|_| Error::InvalidTimestamp {
                    timestamp: timestamp_str.to_string(),
                })?;

            let datetime =
                DateTime::from_timestamp(timestamp, 0).ok_or_else(|| Error::InvalidTimestamp {
                    timestamp: timestamp_str.to_string(),
                })?;

            let (redacted_command, was_redacted) = if self.config.enable_redaction {
                let original = command.to_string();
                let redacted = self.redaction_engine.redact(command)?;
                (redacted.clone(), redacted != original)
            } else {
                (command.to_string(), false)
            };

            Ok(Some(HistoryEntry {
                command: redacted_command,
                timestamp: datetime,
                directory: "<imported>".to_string(),
                redacted: was_redacted,
                original: None,
            }))
        } else {
            Ok(None)
        }
    }

    /// Parse a Bash history entry
    fn parse_bash_entry(&self, line: &str) -> Result<Option<HistoryEntry>> {
        // Bash history is usually just the command, no timestamp
        if line.starts_with('#') {
            return Ok(None); // Skip comments
        }

        let (redacted_command, was_redacted) = if self.config.enable_redaction {
            let original = line.to_string();
            let redacted = self.redaction_engine.redact(line)?;
            (redacted.clone(), redacted != original)
        } else {
            (line.to_string(), false)
        };

        Ok(Some(HistoryEntry {
            command: redacted_command,
            timestamp: Utc::now(), // No timestamp available
            directory: "<imported>".to_string(),
            redacted: was_redacted,
            original: None,
        }))
    }

    /// Parse a Fish history entry
    fn parse_fish_entry(&self, line: &str) -> Result<Option<HistoryEntry>> {
        // Fish format: "- cmd: command\n  when: timestamp\n  paths: [...]"
        // This is a simplified parser for the most common case
        if line.starts_with("- cmd: ") {
            let command = &line[7..]; // Remove "- cmd: "

            let (redacted_command, was_redacted) = if self.config.enable_redaction {
                let original = command.to_string();
                let redacted = self.redaction_engine.redact(command)?;
                (redacted.clone(), redacted != original)
            } else {
                (command.to_string(), false)
            };

            Ok(Some(HistoryEntry {
                command: redacted_command,
                timestamp: Utc::now(), // Would need to parse next lines for timestamp
                directory: "<imported>".to_string(),
                redacted: was_redacted,
                original: None,
            }))
        } else {
            Ok(None)
        }
    }

    /// Check if an entry is a duplicate
    fn is_duplicate(&self, entry: &HistoryEntry) -> Result<bool> {
        // Read the last few entries to check for duplicates
        let file = File::open(&self.history_file)?;
        let reader = BufReader::new(file);
        let mut recent_commands = Vec::new();

        // Only check the last 100 entries for performance
        let lines: Vec<String> = reader.lines().collect::<std::result::Result<Vec<_>, _>>()?;
        for line in lines.iter().rev().take(100) {
            let line = line;
            if let Some(parsed_entry) = self.parse_entry(&line)? {
                recent_commands.push(parsed_entry.command);
            }
        }

        Ok(recent_commands.contains(&entry.command))
    }

    /// Update statistics
    fn update_stats(&mut self) -> Result<()> {
        let entries = self.get_entries()?;
        let mut unique_commands = HashSet::new();
        let mut common_directories = HashMap::new();
        let mut redacted_count = 0;

        for entry in &entries {
            unique_commands.insert(entry.command.clone());
            *common_directories
                .entry(entry.directory.clone())
                .or_insert(0) += 1;
            if entry.redacted {
                redacted_count += 1;
            }
        }

        self.stats.total_entries = entries.len();
        self.stats.unique_commands = unique_commands.len();
        self.stats.redacted_entries = redacted_count;
        self.stats.common_directories = common_directories;

        Ok(())
    }

    /// Update statistics for a single entry
    fn update_stats_for_entry(&mut self, entry: &HistoryEntry) {
        self.stats.total_entries += 1;
        if entry.redacted {
            self.stats.redacted_entries += 1;
        }
        *self
            .stats
            .common_directories
            .entry(entry.directory.clone())
            .or_insert(0) += 1;
    }
}

impl HistoryEntry {
    /// Create a new history entry
    pub fn new(command: String, timestamp: DateTime<Utc>, directory: String) -> Self {
        Self {
            command,
            timestamp,
            directory,
            redacted: false,
            original: None,
        }
    }

    /// Get the command as a string for display
    pub fn display_command(&self) -> &str {
        &self.command
    }

    /// Get formatted timestamp
    pub fn formatted_timestamp(&self) -> String {
        self.timestamp.format("%Y-%m-%d %H:%M:%S").to_string()
    }

    /// Get relative directory (basename)
    pub fn relative_directory(&self) -> String {
        PathBuf::from(&self.directory)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    }
}

/// Conversion from database CommandEntry to HistoryEntry
impl From<crate::database::CommandEntry> for HistoryEntry {
    fn from(cmd: crate::database::CommandEntry) -> Self {
        Self {
            command: cmd.command,
            timestamp: cmd.timestamp,
            directory: cmd.directory,
            redacted: cmd.redacted,
            original: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use tempfile::NamedTempFile;

    fn test_config() -> Config {
        let temp_file = NamedTempFile::new().unwrap();
        let mut config = Config::default();
        config.history_file = temp_file.path().to_path_buf();
        config.max_entries = 1000;
        config
    }

    #[test]
    fn test_history_manager_creation() {
        let config = test_config();
        let manager = HistoryManager::new(config);
        assert!(manager.is_ok());
    }

    #[test]
    fn test_log_command() {
        let config = test_config();
        let mut manager = HistoryManager::new(config).unwrap();

        let result = manager.log_command("echo hello world");
        assert!(result.is_ok());

        let entries = manager.get_entries().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].command, "echo hello world");
    }

    #[test]
    fn test_redaction_in_logging() {
        let config = test_config();
        let mut manager = HistoryManager::new(config).unwrap();

        manager.log_command("password=secret123").unwrap();

        let entries = manager.get_entries().unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].command.contains("<redacted>"));
        assert!(!entries[0].command.contains("secret123"));
    }

    #[test]
    fn test_duplicate_filtering() {
        let mut config = test_config();
        config.shell_integration.log_duplicates = false;
        let mut manager = HistoryManager::new(config).unwrap();

        manager.log_command("echo hello").unwrap();
        manager.log_command("echo hello").unwrap(); // Duplicate
        manager.log_command("echo world").unwrap();

        let entries = manager.get_entries().unwrap();
        assert_eq!(entries.len(), 2); // Should have filtered out the duplicate
    }

    #[test]
    fn test_search() {
        let config = test_config();
        let mut manager = HistoryManager::new(config).unwrap();

        manager.log_command("echo hello").unwrap();
        manager.log_command("ls -la").unwrap();
        manager.log_command("echo world").unwrap();

        let results = manager.search("echo", None).unwrap();
        assert_eq!(results.len(), 2);

        let results = manager.search("ls", None).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_zsh_entry_parsing() {
        let config = test_config();
        let manager = HistoryManager::new(config).unwrap();

        let entry = manager
            .parse_zsh_entry(": 1609786800:0;echo hello world")
            .unwrap();
        assert!(entry.is_some());

        let entry = entry.unwrap();
        assert_eq!(entry.command, "echo hello world");
    }

    #[test]
    fn test_history_stats() {
        let config = test_config();
        let mut manager = HistoryManager::new(config).unwrap();

        manager.log_command("echo hello").unwrap();
        manager.log_command("password=secret").unwrap();
        manager.log_command("ls -la").unwrap();

        let stats = manager.get_stats();
        assert_eq!(stats.total_entries, 3);
        assert_eq!(stats.redacted_entries, 1);
        assert_eq!(stats.unique_commands, 3);
    }

    #[test]
    fn test_clear_history() {
        let config = test_config();
        let mut manager = HistoryManager::new(config).unwrap();

        manager.log_command("echo hello").unwrap();
        manager.log_command("ls -la").unwrap();

        assert_eq!(manager.get_entries().unwrap().len(), 2);

        manager.clear().unwrap();
        assert_eq!(manager.get_entries().unwrap().len(), 0);
    }

    #[test]
    fn test_trim_history() {
        let mut config = test_config();
        config.max_entries = 2;
        let mut manager = HistoryManager::new(config).unwrap();

        manager.log_command("command1").unwrap();
        manager.log_command("command2").unwrap();
        manager.log_command("command3").unwrap(); // This should trigger trimming

        let entries = manager.get_entries().unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].command, "command2");
        assert_eq!(entries[1].command, "command3");
    }
}
