//! Mortimer - Enhanced shell history manager with sensitive data redaction
//!
//! This is the main entry point for the Mortimer command-line application.
//! It initializes the application and handles errors gracefully.

use mortimer::cli;
use mortimer::error::Result;
use std::process;

fn main() {
    // Run the application and handle errors
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}

fn run() -> Result<()> {
    cli::run()
}
