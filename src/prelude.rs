//! Prelude module for zam
//!
//! This module re-exports commonly used types and traits to reduce
//! boilerplate imports throughout the codebase.
//!
//! # Usage
//!
//! ```rust
//! use zam::prelude::*;
//! ```

pub use crate::config::Config;
pub use crate::error::{Error, Result};
pub use crate::types::{CommandId, HostId, SessionId};

// Re-export commonly used external types
pub use chrono::{DateTime, Utc};
