//! Command handler implementations for Mortimer CLI
//!
//! This module organizes command handlers into logical groups:
//! - `basic`: Basic commands (log, search, recent, frequent)
//! - `import_export`: Import and export handlers
//! - `database`: Database-specific handlers (migrate, merge, tokens, hosts, sessions)
//! - `config`: Configuration and shell integration handlers
//! - `util`: Utility functions for handlers

mod basic;
mod config;
mod database;
mod import_export;
mod manage;
mod shell_integration;

pub use basic::*;
pub use config::*;
pub use database::*;
pub use import_export::*;
pub use manage::*;
pub use shell_integration::*;
