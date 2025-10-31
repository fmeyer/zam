//! Database management for Mortimer
//!
//! This module provides SQLite-based storage for command history with support for:
//! - Multi-host history tracking
//! - Session management
//! - Token/password storage for retrieval
//! - Migration from legacy .mhist files

use crate::error::Result;
use crate::types::{CommandId, HostId, SessionId};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use uuid::Uuid;

/// Represents a host in the database
#[derive(Debug, Clone)]
pub struct Host {
    pub id: HostId,
    pub hostname: String,
    pub created_at: DateTime<Utc>,
}

/// Represents a shell session
#[derive(Debug, Clone)]
pub struct Session {
    pub id: SessionId,
    pub host_id: HostId,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
}

/// Represents a command entry in the database
#[derive(Debug, Clone, serde::Serialize)]
pub struct CommandEntry {
    pub id: CommandId,
    pub session_id: SessionId,
    pub command: String,
    pub timestamp: DateTime<Utc>,
    pub directory: String,
    pub redacted: bool,
    pub exit_code: Option<i32>,
}

/// Represents a redacted token that can be retrieved
#[derive(Debug, Clone)]
pub struct Token {
    pub id: i64,
    pub command_id: CommandId,
    pub token_type: String, // e.g., "password", "api_key", "token"
    pub placeholder: String,
    pub original_value: String,
    pub created_at: DateTime<Utc>,
}

/// Statistics about the database
#[derive(Debug, Clone, Default)]
pub struct DatabaseStats {
    pub total_commands: usize,
    pub total_sessions: usize,
    pub total_hosts: usize,
    pub redacted_commands: usize,
    pub stored_tokens: usize,
    pub oldest_entry: Option<DateTime<Utc>>,
    pub newest_entry: Option<DateTime<Utc>>,
}

/// Main database manager
pub struct Database {
    conn: Connection,
    current_host_id: HostId,
    current_session_id: Option<SessionId>,
}

impl Database {
    /// Create a new database connection and initialize schema
    #[must_use = "Database connection must be used"]
    pub fn new(db_path: &Path) -> Result<Self> {
        // Create parent directory if it doesn't exist
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(db_path)?;

        // Enable foreign keys
        conn.execute("PRAGMA foreign_keys = ON", [])?;

        let mut db = Self {
            conn,
            current_host_id: HostId::new(0),
            current_session_id: None,
        };

        db.initialize_schema()?;
        db.ensure_current_host()?;

        Ok(db)
    }

    /// Initialize database schema
    fn initialize_schema(&self) -> Result<()> {
        // Hosts table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS hosts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                hostname TEXT NOT NULL UNIQUE,
                created_at TEXT NOT NULL
            )",
            [],
        )?;

        // Sessions table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                host_id INTEGER NOT NULL,
                started_at TEXT NOT NULL,
                ended_at TEXT,
                FOREIGN KEY (host_id) REFERENCES hosts(id) ON DELETE CASCADE
            )",
            [],
        )?;

        // Commands table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS commands (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                command TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                directory TEXT NOT NULL,
                redacted INTEGER NOT NULL DEFAULT 0,
                exit_code INTEGER,
                FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
            )",
            [],
        )?;

        // Tokens table - stores redacted values for retrieval
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS tokens (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                command_id INTEGER NOT NULL,
                token_type TEXT NOT NULL,
                placeholder TEXT NOT NULL,
                original_value TEXT NOT NULL,
                created_at TEXT NOT NULL,
                FOREIGN KEY (command_id) REFERENCES commands(id) ON DELETE CASCADE
            )",
            [],
        )?;

        // Create indices for common queries
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_commands_timestamp ON commands(timestamp DESC)",
            [],
        )?;

        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_commands_session ON commands(session_id)",
            [],
        )?;

        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_commands_directory ON commands(directory)",
            [],
        )?;

        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_tokens_command ON tokens(command_id)",
            [],
        )?;

        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_sessions_host ON sessions(host_id)",
            [],
        )?;

        Ok(())
    }

    /// Ensure the current host exists in the database
    fn ensure_current_host(&mut self) -> Result<()> {
        let hostname = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        // Try to find existing host
        let host_id: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM hosts WHERE hostname = ?1",
                params![hostname],
                |row| row.get(0),
            )
            .optional()?;

        self.current_host_id = if let Some(id) = host_id {
            HostId::new(id)
        } else {
            // Insert new host
            let now = Utc::now().to_rfc3339();
            self.conn.execute(
                "INSERT INTO hosts (hostname, created_at) VALUES (?1, ?2)",
                params![hostname, now],
            )?;
            HostId::new(self.conn.last_insert_rowid())
        };

        Ok(())
    }

    /// Start a new session
    pub fn start_session(&mut self) -> Result<String> {
        let session_id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        self.conn.execute(
            "INSERT INTO sessions (id, host_id, started_at) VALUES (?1, ?2, ?3)",
            params![session_id, self.current_host_id.as_i64(), now],
        )?;

        self.current_session_id = Some(SessionId::new(session_id.clone()));
        Ok(session_id)
    }

    /// End the current session
    pub fn end_session(&mut self, session_id: &str) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "UPDATE sessions SET ended_at = ?1 WHERE id = ?2",
            params![now, session_id],
        )?;

        if self.current_session_id.as_deref() == Some(session_id) {
            self.current_session_id = None;
        }

        Ok(())
    }

    /// Get or create a session for the current shell
    pub fn ensure_session(&mut self) -> Result<String> {
        if let Some(ref session_id) = self.current_session_id {
            Ok(session_id.as_str().to_string())
        } else {
            self.start_session()
        }
    }

    /// Add a command to the database
    pub fn add_command(
        &mut self,
        command: &str,
        directory: &str,
        timestamp: DateTime<Utc>,
        redacted: bool,
        exit_code: Option<i32>,
    ) -> Result<i64> {
        let session_id = self.ensure_session()?;
        let timestamp_str = timestamp.to_rfc3339();

        self.conn.execute(
            "INSERT INTO commands (session_id, command, timestamp, directory, redacted, exit_code)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                session_id,
                command,
                timestamp_str,
                directory,
                redacted as i32,
                exit_code
            ],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    /// Store a redacted token for later retrieval
    pub fn store_token(
        &self,
        command_id: i64,
        token_type: &str,
        placeholder: &str,
        original_value: &str,
    ) -> Result<i64> {
        let now = Utc::now().to_rfc3339();

        self.conn.execute(
            "INSERT INTO tokens (command_id, token_type, placeholder, original_value, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![command_id, token_type, placeholder, original_value, now],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    /// Get tokens for a specific command
    #[must_use = "Token query results should be used"]
    pub fn get_tokens_for_command(&self, command_id: CommandId) -> Result<Vec<Token>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, command_id, token_type, placeholder, original_value, created_at
             FROM tokens WHERE command_id = ?1",
        )?;

        let tokens = stmt
            .query_map(params![command_id.as_i64()], |row| {
                Ok(Token {
                    id: row.get(0)?,
                    command_id: CommandId::new(row.get(1)?),
                    token_type: row.get(2)?,
                    placeholder: row.get(3)?,
                    original_value: row.get(4)?,
                    created_at: row
                        .get::<_, String>(5)?
                        .parse()
                        .unwrap_or_else(|_| Utc::now()),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(tokens)
    }

    /// Get tokens by session
    pub fn get_tokens_by_session(&self, session_id: &str) -> Result<Vec<Token>> {
        let mut stmt = self.conn.prepare(
            "SELECT t.id, t.command_id, t.token_type, t.placeholder, t.original_value, t.created_at
             FROM tokens t
             JOIN commands c ON t.command_id = c.id
             WHERE c.session_id = ?1
             ORDER BY t.created_at DESC",
        )?;

        let tokens = stmt
            .query_map(params![session_id], |row| {
                Ok(Token {
                    id: row.get(0)?,
                    command_id: row.get(1)?,
                    token_type: row.get(2)?,
                    placeholder: row.get(3)?,
                    original_value: row.get(4)?,
                    created_at: row
                        .get::<_, String>(5)?
                        .parse()
                        .unwrap_or_else(|_| Utc::now()),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(tokens)
    }

    /// Get tokens by directory
    pub fn get_tokens_by_directory(&self, directory: &str) -> Result<Vec<Token>> {
        let mut stmt = self.conn.prepare(
            "SELECT t.id, t.command_id, t.token_type, t.placeholder, t.original_value, t.created_at
             FROM tokens t
             JOIN commands c ON t.command_id = c.id
             WHERE c.directory = ?1
             ORDER BY t.created_at DESC",
        )?;

        let tokens = stmt
            .query_map(params![directory], |row| {
                Ok(Token {
                    id: row.get(0)?,
                    command_id: row.get(1)?,
                    token_type: row.get(2)?,
                    placeholder: row.get(3)?,
                    original_value: row.get(4)?,
                    created_at: row
                        .get::<_, String>(5)?
                        .parse()
                        .unwrap_or_else(|_| Utc::now()),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(tokens)
    }

    /// Search commands
    #[must_use = "Search results should be used"]
    pub fn search_commands(
        &self,
        query: &str,
        directory_filter: Option<&str>,
        host_filter: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<CommandEntry>> {
        let mut sql = String::from(
            "SELECT c.id, c.session_id, c.command, c.timestamp, c.directory, c.redacted, c.exit_code
             FROM commands c
             JOIN sessions s ON c.session_id = s.id
             JOIN hosts h ON s.host_id = h.id
             WHERE c.command LIKE ?1",
        );

        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(format!("%{}%", query))];

        if let Some(dir) = directory_filter {
            sql.push_str(" AND c.directory LIKE ?");
            params.push(Box::new(format!("%{}%", dir)));
        }

        if let Some(host) = host_filter {
            sql.push_str(" AND h.hostname = ?");
            params.push(Box::new(host.to_string()));
        }

        sql.push_str(" ORDER BY c.timestamp DESC");

        if let Some(lim) = limit {
            sql.push_str(" LIMIT ?");
            params.push(Box::new(lim as i64));
        }

        let mut stmt = self.conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|b| b.as_ref()).collect();

        let commands = stmt
            .query_map(param_refs.as_slice(), |row| {
                Ok(CommandEntry {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    command: row.get(2)?,
                    timestamp: row
                        .get::<_, String>(3)?
                        .parse()
                        .unwrap_or_else(|_| Utc::now()),
                    directory: row.get(4)?,
                    redacted: row.get::<_, i32>(5)? != 0,
                    exit_code: row.get(6)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(commands)
    }

    /// Get recent commands
    #[must_use = "Query results should be used"]
    pub fn get_recent_commands(&self, limit: usize) -> Result<Vec<CommandEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, session_id, command, timestamp, directory, redacted, exit_code
             FROM commands
             ORDER BY timestamp DESC
             LIMIT ?1",
        )?;

        let commands = stmt
            .query_map(params![limit as i64], |row| {
                Ok(CommandEntry {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    command: row.get(2)?,
                    timestamp: row
                        .get::<_, String>(3)?
                        .parse()
                        .unwrap_or_else(|_| Utc::now()),
                    directory: row.get(4)?,
                    redacted: row.get::<_, i32>(5)? != 0,
                    exit_code: row.get(6)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(commands)
    }

    /// Get all commands (for export/migration)
    #[must_use = "Query results should be used"]
    pub fn get_all_commands(&self) -> Result<Vec<CommandEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, session_id, command, timestamp, directory, redacted, exit_code
             FROM commands
             ORDER BY timestamp ASC",
        )?;

        let commands = stmt
            .query_map([], |row| {
                Ok(CommandEntry {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    command: row.get(2)?,
                    timestamp: row
                        .get::<_, String>(3)?
                        .parse()
                        .unwrap_or_else(|_| Utc::now()),
                    directory: row.get(4)?,
                    redacted: row.get::<_, i32>(5)? != 0,
                    exit_code: row.get(6)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(commands)
    }

    /// Get database statistics
    pub fn get_stats(&self) -> Result<DatabaseStats> {
        let total_commands: usize =
            self.conn
                .query_row("SELECT COUNT(*) FROM commands", [], |row| row.get(0))?;

        let total_sessions: usize =
            self.conn
                .query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))?;

        let total_hosts: usize = self
            .conn
            .query_row("SELECT COUNT(*) FROM hosts", [], |row| row.get(0))?;

        let redacted_commands: usize = self.conn.query_row(
            "SELECT COUNT(*) FROM commands WHERE redacted = 1",
            [],
            |row| row.get(0),
        )?;

        let stored_tokens: usize =
            self.conn
                .query_row("SELECT COUNT(*) FROM tokens", [], |row| row.get(0))?;

        let oldest_entry: Option<String> = self
            .conn
            .query_row(
                "SELECT timestamp FROM commands ORDER BY timestamp ASC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()?;

        let newest_entry: Option<String> = self
            .conn
            .query_row(
                "SELECT timestamp FROM commands ORDER BY timestamp DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()?;

        Ok(DatabaseStats {
            total_commands,
            total_sessions,
            total_hosts,
            redacted_commands,
            stored_tokens,
            oldest_entry: oldest_entry.and_then(|s| s.parse().ok()),
            newest_entry: newest_entry.and_then(|s| s.parse().ok()),
        })
    }

    /// Get all hosts
    pub fn get_hosts(&self) -> Result<Vec<Host>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, hostname, created_at FROM hosts ORDER BY hostname")?;

        let hosts = stmt
            .query_map([], |row| {
                Ok(Host {
                    id: row.get(0)?,
                    hostname: row.get(1)?,
                    created_at: row
                        .get::<_, String>(2)?
                        .parse()
                        .unwrap_or_else(|_| Utc::now()),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(hosts)
    }

    /// Get sessions for a host
    pub fn get_sessions_for_host(&self, host_id: HostId) -> Result<Vec<Session>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, host_id, started_at, ended_at
             FROM sessions
             WHERE host_id = ?1
             ORDER BY started_at DESC",
        )?;

        let sessions = stmt
            .query_map(params![host_id.as_i64()], |row| {
                Ok(Session {
                    id: SessionId::new(row.get(0)?),
                    host_id: HostId::new(row.get(1)?),
                    started_at: row
                        .get::<_, String>(2)?
                        .parse()
                        .unwrap_or_else(|_| Utc::now()),
                    ended_at: row
                        .get::<_, Option<String>>(3)?
                        .and_then(|s| s.parse().ok()),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(sessions)
    }

    /// Import from legacy .mhist file format
    /// Handles multiline commands properly
    pub fn import_from_mhist(&mut self, mhist_path: &Path) -> Result<usize> {
        let content = std::fs::read_to_string(mhist_path)?;
        let mut imported_count = 0;
        let mut current_entry: Option<(DateTime<Utc>, String, String)> = None;

        for line in content.lines() {
            // Check if this is a new entry (starts with timestamp pattern)
            if let Some(entry_parts) = Self::parse_mhist_line(line) {
                // Save previous entry if exists
                if let Some((timestamp, directory, command)) = current_entry.take() {
                    self.add_command(&command, &directory, timestamp, false, None)?;
                    imported_count += 1;
                }

                // Start new entry
                current_entry = Some(entry_parts);
            } else if let Some((_timestamp, _directory, command)) = current_entry.as_mut() {
                // This is a continuation line (multiline command)
                command.push('\n');
                command.push_str(line.trim());
            }
        }

        // Don't forget the last entry
        if let Some((timestamp, directory, command)) = current_entry {
            self.add_command(&command, &directory, timestamp, false, None)?;
            imported_count += 1;
        }

        Ok(imported_count)
    }

    /// Parse a single .mhist line
    /// Format: "2025-10-27 19:39:35 | /Users/fm/tmp | command"
    fn parse_mhist_line(line: &str) -> Option<(DateTime<Utc>, String, String)> {
        let parts: Vec<&str> = line.splitn(3, " | ").collect();
        if parts.len() != 3 {
            return None;
        }

        let timestamp_str = parts[0].trim();
        let directory = parts[1].trim().to_string();
        let command = parts[2].to_string();

        // Parse timestamp
        let timestamp = chrono::NaiveDateTime::parse_from_str(timestamp_str, "%Y-%m-%d %H:%M:%S")
            .ok()?
            .and_utc();

        Some((timestamp, directory, command))
    }

    /// Import from bash history
    pub fn import_from_bash_history(&mut self, bash_history_path: &Path) -> Result<usize> {
        let content = std::fs::read_to_string(bash_history_path)?;
        let mut imported_count = 0;
        let now = Utc::now();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            self.add_command(line, "<imported>", now, false, None)?;
            imported_count += 1;
        }

        Ok(imported_count)
    }

    /// Import from zsh history
    pub fn import_from_zsh_history(&mut self, zsh_history_path: &Path) -> Result<usize> {
        let content = std::fs::read_to_string(zsh_history_path)?;
        let mut imported_count = 0;

        // Zsh format: ": 1609786800:0;command"
        let re = regex::Regex::new(r"^: (\d+):\d+;(.*)").unwrap();

        for line in content.lines() {
            if let Some(caps) = re.captures(line) {
                let timestamp_str = caps.get(1).unwrap().as_str();
                let command = caps.get(2).unwrap().as_str();

                if let Ok(timestamp_secs) = timestamp_str.parse::<i64>() {
                    if let Some(datetime) = DateTime::from_timestamp(timestamp_secs, 0) {
                        self.add_command(command, "<imported>", datetime, false, None)?;
                        imported_count += 1;
                    }
                }
            }
        }

        Ok(imported_count)
    }

    /// Merge another database into this one
    pub fn merge_from_database(&mut self, other_db_path: &Path) -> Result<usize> {
        let other_conn = Connection::open(other_db_path)?;
        let mut imported_count = 0;

        // Get all commands from the other database
        let mut stmt = other_conn.prepare(
            "SELECT c.command, c.timestamp, c.directory, c.redacted, c.exit_code,
                    s.started_at, h.hostname
             FROM commands c
             JOIN sessions s ON c.session_id = s.id
             JOIN hosts h ON s.host_id = h.id
             ORDER BY c.timestamp ASC",
        )?;

        let commands: Vec<_> = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i32>(3)? != 0,
                    row.get::<_, Option<i32>>(4)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        for (command, timestamp_str, directory, redacted, exit_code) in commands {
            if let Ok(timestamp) = timestamp_str.parse() {
                self.add_command(&command, &directory, timestamp, redacted, exit_code)?;
                imported_count += 1;
            }
        }

        Ok(imported_count)
    }

    /// Clear all data (for testing)
    pub fn clear(&self) -> Result<()> {
        self.conn.execute("DELETE FROM tokens", [])?;
        self.conn.execute("DELETE FROM commands", [])?;
        self.conn.execute("DELETE FROM sessions", [])?;
        self.conn.execute("DELETE FROM hosts", [])?;
        Ok(())
    }

    /// Delete a specific command by ID
    pub fn delete_command(&self, id: CommandId) -> Result<()> {
        self.conn.execute("DELETE FROM commands WHERE id = ?1", [id.0])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_database_creation() {
        let temp_file = NamedTempFile::new().unwrap();
        let db = Database::new(temp_file.path()).unwrap();
        let stats = db.get_stats().unwrap();
        assert_eq!(stats.total_commands, 0);
    }

    #[test]
    fn test_add_command() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut db = Database::new(temp_file.path()).unwrap();

        let cmd_id = db
            .add_command("ls -la", "/home/user", Utc::now(), false, Some(0))
            .unwrap();
        assert!(cmd_id > 0);

        let stats = db.get_stats().unwrap();
        assert_eq!(stats.total_commands, 1);
    }

    #[test]
    fn test_token_storage() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut db = Database::new(temp_file.path()).unwrap();

        let cmd_id = db
            .add_command("echo password123", "/home", Utc::now(), true, None)
            .unwrap();

        db.store_token(cmd_id, "password", "<redacted>", "password123")
            .unwrap();

        let tokens = db.get_tokens_for_command(CommandId::new(cmd_id)).unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].original_value, "password123");
    }

    #[test]
    fn test_mhist_parsing() {
        let line = "2025-10-27 19:39:35 | /Users/fm/tmp | ls -la";
        let result = Database::parse_mhist_line(line);
        assert!(result.is_some());

        let (_, directory, command) = result.unwrap();
        assert_eq!(directory, "/Users/fm/tmp");
        assert_eq!(command, "ls -la");
    }
}
