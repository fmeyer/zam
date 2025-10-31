//! Common backend trait for history storage
//!
//! This module defines the HistoryProvider trait that both file-based
//! and database backends implement, allowing handlers to work with
//! either backend without pattern matching.

use crate::error::Result;
use crate::history::HistoryEntry;

/// Common interface for history storage backends
///
/// This trait is implemented by both `HistoryManager` (file-based)
/// and `HistoryManagerDb` (database) to provide a unified API.
pub trait HistoryProvider {
    /// Get all history entries
    ///
    /// Returns all commands in the history, regardless of backend type.
    #[must_use = "Query results should be used"]
    fn get_entries(&self) -> Result<Vec<HistoryEntry>>;

    /// Get recent N entries
    ///
    /// Returns the most recent commands, with the most recent first.
    #[must_use = "Query results should be used"]
    fn get_recent(&self, count: usize) -> Result<Vec<HistoryEntry>>;

    /// Search history with a query string
    ///
    /// Basic search functionality that works across both backends.
    #[must_use = "Search results should be used"]
    fn search(&self, query: &str) -> Result<Vec<HistoryEntry>>;

    /// Log a command to history
    ///
    /// Stores a command in the history with automatic redaction.
    fn log_command(&mut self, command: &str) -> Result<()>;

    /// Clear all history
    ///
    /// Removes all entries from the history.
    fn clear(&mut self) -> Result<()>;

    /// Delete entries by indices
    ///
    /// Removes specific entries from history by their position.
    /// Indices should be in the order returned by get_entries().
    fn delete_entries(&mut self, indices: &[usize]) -> Result<usize>;
}
