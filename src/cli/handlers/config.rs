//! Configuration and statistics handlers for Mortimer CLI

use crate::cli::args::*;
use crate::cli::{CliApp, HistoryBackend};
use crate::error::Result;
use std::io::{self, Write};

pub fn handle_config(app: &mut CliApp, args: &ConfigArgs) -> Result<()> {
    if args.show {
        let config_json = serde_json::to_string_pretty(&app.config)?;
        println!("{}", config_json);
    } else if args.init {
        let config_path = crate::config::Config::default_config_path()?;
        let config = crate::config::Config::default();
        config.save_to_path(&config_path)?;
        println!("Configuration initialized at {}", config_path.display());
    } else if args.validate {
        match app.config.validate() {
            Ok(_) => println!("Configuration is valid"),
            Err(e) => println!("Configuration validation failed: {}", e),
        }
    } else {
        println!("Use --show, --init, or --validate");
    }

    Ok(())
}

pub fn handle_stats(app: &mut CliApp, args: &StatsArgs) -> Result<()> {
    match &mut app.backend {
        HistoryBackend::File(mgr) => {
            let stats = mgr.get_stats()?;

            println!("History Statistics (File-based)");
            println!("================================");
            println!("Backend: File (~/.mhist)");
            println!("Total entries: {}", stats.total_entries);
            println!("Unique commands: {}", stats.unique_commands);
            println!("Redacted entries: {}", stats.redacted_entries);
            println!("Duplicates filtered: {}", stats.duplicates_filtered);

            if args.redaction {
                println!("\nRedaction Statistics");
                println!("===================");
                println!(
                    "Total commands processed: {}",
                    stats.redaction_stats.total_commands
                );
                println!(
                    "Commands redacted: {}",
                    stats.redaction_stats.redacted_commands
                );
                println!(
                    "Environment variables redacted: {}",
                    stats.redaction_stats.env_vars_redacted
                );

                if !stats.redaction_stats.patterns_matched.is_empty() {
                    println!("\nPatterns matched:");
                    for (pattern, count) in &stats.redaction_stats.patterns_matched {
                        println!("  {}: {}", pattern, count);
                    }
                }
            }

            if args.directories {
                println!("\nDirectory Statistics");
                println!("===================");
                let mut dirs: Vec<_> = stats.common_directories.iter().collect();
                dirs.sort_by(|a, b| b.1.cmp(a.1));
                for (dir, count) in dirs.iter().take(10) {
                    println!("  {}: {}", dir, count);
                }
            }
        }
        HistoryBackend::Database(mgr) => {
            let stats = mgr.get_stats()?;

            println!("History Statistics (Database)");
            println!("==============================");
            println!("Backend: SQLite Database");
            println!("Total commands: {}", stats.total_commands);
            println!("Total sessions: {}", stats.total_sessions);
            println!("Total hosts: {}", stats.total_hosts);
            println!("Redacted commands: {}", stats.redacted_commands);
            println!("Stored tokens: {}", stats.stored_tokens);

            if let Some(oldest) = stats.oldest_entry {
                println!("Oldest entry: {}", oldest.format("%Y-%m-%d %H:%M:%S"));
            }
            if let Some(newest) = stats.newest_entry {
                println!("Newest entry: {}", newest.format("%Y-%m-%d %H:%M:%S"));
            }
        }
    }

    Ok(())
}

pub fn handle_status(app: &mut CliApp) -> Result<()> {
    println!("Mortimer Status");
    println!("===============\n");

    // Show backend type
    match &app.backend {
        HistoryBackend::File(_) => {
            println!("Backend: File-based");
            println!("Storage: {}", app.config.history_file.display());
            println!("Type: Legacy .mhist format\n");

            if app.config.history_file.with_extension("db").exists() {
                println!(
                    "⚠️  Note: A database file exists at {}",
                    app.config.history_file.with_extension("db").display()
                );
                println!("   To use it, run commands with --use-db flag");
                println!("   Or delete the .mhist file to auto-switch\n");
            }
        }
        HistoryBackend::Database(_) => {
            println!("Backend: SQLite Database");
            println!(
                "Storage: {}",
                app.config.history_file.with_extension("db").display()
            );
            println!("Type: Multi-host, session-aware\n");

            if app.config.history_file.exists() {
                println!("ℹ️  Note: Legacy .mhist file still exists");
                println!("   You can safely delete it after verifying migration\n");
            }
        }
    }

    // Show configuration
    println!("Configuration:");
    println!("  Redaction enabled: {}", app.config.enable_redaction);
    println!(
        "  Max entries: {}",
        if app.config.max_entries == 0 {
            "unlimited".to_string()
        } else {
            app.config.max_entries.to_string()
        }
    );
    println!("  Auto-log: {}", app.config.shell_integration.auto_log);
    println!(
        "  Log duplicates: {}",
        app.config.shell_integration.log_duplicates
    );

    if !app.config.shell_integration.exclude_commands.is_empty() {
        println!(
            "  Excluded commands: {}",
            app.config.shell_integration.exclude_commands.len()
        );
    }

    println!();

    // Show quick stats
    match &mut app.backend {
        HistoryBackend::File(mgr) => match mgr.get_stats() {
            Ok(stats) => {
                println!("Quick Stats:");
                println!("  Total entries: {}", stats.total_entries);
                println!("  Unique commands: {}", stats.unique_commands);
                println!("  Redacted entries: {}", stats.redacted_entries);
            }
            Err(e) => {
                eprintln!("Error getting stats: {}", e);
            }
        },
        HistoryBackend::Database(mgr) => match mgr.get_stats() {
            Ok(stats) => {
                println!("Quick Stats:");
                println!("  Total commands: {}", stats.total_commands);
                println!("  Total sessions: {}", stats.total_sessions);
                println!("  Total hosts: {}", stats.total_hosts);
                println!("  Redacted commands: {}", stats.redacted_commands);
                println!("  Stored tokens: {}", stats.stored_tokens);

                if let Some(oldest) = stats.oldest_entry {
                    println!("  Oldest entry: {}", oldest.format("%Y-%m-%d"));
                }
                if let Some(newest) = stats.newest_entry {
                    println!("  Newest entry: {}", newest.format("%Y-%m-%d"));
                }
            }
            Err(e) => {
                eprintln!("Error getting stats: {}", e);
            }
        },
    }

    println!("\nFor detailed statistics, run: mortimer stats");

    Ok(())
}

pub fn handle_clear(app: &mut CliApp, args: &ClearArgs) -> Result<()> {
    if !args.force {
        print!("Are you sure you want to clear the history? (y/N): ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Aborted");
            return Ok(());
        }
    }

    match &mut app.backend {
        HistoryBackend::File(mgr) => mgr.clear()?,
        HistoryBackend::Database(mgr) => mgr.clear()?,
    }

    if !app.quiet {
        println!("History cleared successfully");
    }

    Ok(())
}
