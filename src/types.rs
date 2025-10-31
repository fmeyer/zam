//! Type definitions for Mortimer
//!
//! This module provides type-safe wrappers around primitive types
//! to prevent accidental misuse of IDs and other domain-specific values.

use rusqlite::types::{FromSql, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::Deref;

/// A type-safe wrapper for command IDs
///
/// Prevents accidentally passing a host ID where a command ID is expected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CommandId(pub i64);

impl CommandId {
    /// Create a new CommandId
    pub fn new(id: i64) -> Self {
        Self(id)
    }

    /// Get the inner i64 value
    pub fn as_i64(self) -> i64 {
        self.0
    }

    /// Get a reference to the inner i64 value
    pub fn as_ref(&self) -> &i64 {
        &self.0
    }
}

impl From<i64> for CommandId {
    fn from(id: i64) -> Self {
        Self(id)
    }
}

impl From<CommandId> for i64 {
    fn from(id: CommandId) -> Self {
        id.0
    }
}

impl fmt::Display for CommandId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl ToSql for CommandId {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.0))
    }
}

impl FromSql for CommandId {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        i64::column_result(value).map(CommandId::new)
    }
}

/// A type-safe wrapper for host IDs
///
/// Prevents accidentally passing a command ID where a host ID is expected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HostId(pub i64);

impl HostId {
    /// Create a new HostId
    pub fn new(id: i64) -> Self {
        Self(id)
    }

    /// Get the inner i64 value
    pub fn as_i64(self) -> i64 {
        self.0
    }

    /// Get a reference to the inner i64 value
    pub fn as_ref(&self) -> &i64 {
        &self.0
    }
}

impl From<i64> for HostId {
    fn from(id: i64) -> Self {
        Self(id)
    }
}

impl From<HostId> for i64 {
    fn from(id: HostId) -> Self {
        id.0
    }
}

impl fmt::Display for HostId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl ToSql for HostId {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.0))
    }
}

impl FromSql for HostId {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        i64::column_result(value).map(HostId::new)
    }
}

/// A type-safe wrapper for session IDs
///
/// Session IDs are UUIDs stored as strings.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionId(pub String);

impl SessionId {
    /// Create a new SessionId
    pub fn new(id: String) -> Self {
        Self(id)
    }

    /// Get a reference to the inner String
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume self and return the inner String
    pub fn into_string(self) -> String {
        self.0
    }
}

impl From<String> for SessionId {
    fn from(id: String) -> Self {
        Self(id)
    }
}

impl From<SessionId> for String {
    fn from(id: SessionId) -> Self {
        id.0
    }
}

impl AsRef<str> for SessionId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Deref for SessionId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ToSql for SessionId {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.0.as_str()))
    }
}

impl FromSql for SessionId {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        String::column_result(value).map(SessionId::new)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_id_creation() {
        let id = CommandId::new(42);
        assert_eq!(id.as_i64(), 42);
    }

    #[test]
    fn test_command_id_from_conversion() {
        let id: CommandId = 42.into();
        assert_eq!(id.as_i64(), 42);
        let raw: i64 = id.into();
        assert_eq!(raw, 42);
    }

    #[test]
    fn test_host_id_creation() {
        let id = HostId::new(100);
        assert_eq!(id.as_i64(), 100);
    }

    #[test]
    fn test_session_id_creation() {
        let uuid = "550e8400-e29b-41d4-a716-446655440000".to_string();
        let id = SessionId::new(uuid.clone());
        assert_eq!(id.as_str(), uuid);
    }

    #[test]
    fn test_session_id_as_ref() {
        let uuid = "550e8400-e29b-41d4-a716-446655440000".to_string();
        let id = SessionId::new(uuid.clone());
        let str_ref: &str = id.as_ref();
        assert_eq!(str_ref, uuid);
    }

    #[test]
    fn test_display_implementations() {
        let cmd_id = CommandId::new(42);
        assert_eq!(format!("{}", cmd_id), "42");

        let host_id = HostId::new(100);
        assert_eq!(format!("{}", host_id), "100");

        let session_id = SessionId::new("test-uuid".to_string());
        assert_eq!(format!("{}", session_id), "test-uuid");
    }

    #[test]
    fn test_ids_are_not_interchangeable() {
        // This won't compile, which is exactly what we want!
        // let cmd_id = CommandId::new(42);
        // let host_id: HostId = cmd_id; // Error: mismatched types
    }
}
