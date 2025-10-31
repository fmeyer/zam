//! Mortimer - Enhanced shell history manager with sensitive data redaction
//!
//! This is the main entry point for the Mortimer command-line application.
//! It initializes the application and handles errors gracefully.

use mortimer::cli;
use mortimer::error::Result;
use std::process;
use tracing_subscriber::{fmt, EnvFilter};

fn main() {
    // Initialize tracing based on RUST_LOG environment variable
    // Default to "info" if not set
    let env_filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("warn"))
        .unwrap();

    fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .with_writer(std::io::stderr)
        .init();

    // Run the application and handle errors
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}

fn run() -> Result<()> {
    cli::run()
}
