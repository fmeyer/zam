//! Basic command handlers for Mortimer CLI

use crate::cli::args::*;
use crate::cli::{CliApp, HistoryBackend};
use crate::error::{Error, Result};
use crate::search::SearchQuery;

pub fn handle_log(app: &mut CliApp, args: &LogArgs) -> Result<()> {
    if !app.quiet {
        app.verbose_println(&format!("Logging command: {}", args.command));
    }

    // Handle timestamp
    let timestamp = if let Some(ts) = args.timestamp {
        Some(
            chrono::DateTime::from_timestamp(ts, 0).ok_or_else(|| Error::InvalidTimestamp {
                timestamp: ts.to_string(),
            })?,
        )
    } else {
        None
    };

    // Log the command
    if timestamp.is_none() {
        // Use trait method for simple case
        app.provider_mut().log_command(&args.command)?;
    } else {
        // Use backend-specific methods for timestamp support
        match &mut app.backend {
            HistoryBackend::File(mgr) => {
                mgr.log_command_with_timestamp(&args.command, timestamp)?;
            }
            HistoryBackend::Database(mgr) => {
                mgr.log_command_with_timestamp(&args.command, timestamp, None)?;
            }
        }
    }

    if !app.quiet {
        app.verbose_println("Command loggeed successfully");
    }

    Ok(())
}

pub fn handle_search(app: &mut CliApp, args: &SearchArgs) -> Result<()> {
    // Get entries based on backend
    let entries = match &app.backend {
        HistoryBackend::File(mgr) => mgr.get_entries()?,
        HistoryBackend::Database(mgr) => {
            // For database, use direct search if no complex filters
            if args.since.is_none() && args.before.is_none() && !args.regex && !args.exact {
                let db_results = mgr.search(
                    &args.term,
                    args.directory.as_deref(),
                    None,
                    Some(args.limit),
                )?;

                // Display results
                for result in &db_results {
                    let mut output = String::new();

                    if args.timestamps {
                        output.push_str(&format!(
                            "{} ",
                            result.timestamp.format("%Y-%m-%d %H:%M:%S")
                        ));
                    }

                    if args.show_dirs {
                        output.push_str(&format!("{} ", result.directory));
                    }

                    output.push_str(&result.command);
                    println!("{}", output);
                }

                if !app.quiet {
                    println!("\nFound {} results", db_results.len());
                }
                return Ok(());
            }

            // Otherwise, get all and use search engine
            mgr.get_all_commands()?
                .into_iter()
                .map(Into::into)
                .collect()
        }
    };

    // Build search query
    let mut query = SearchQuery::new(args.term.clone());

    if let Some(dir) = &args.directory {
        query = query.with_directory(dir.clone());
    }

    if args.exact {
        query.fuzzy = false;
    }

    if args.case_sensitive {
        query.case_sensitive = true;
    }

    if args.regex {
        query = query.regex();
    }

    if args.redacted_only {
        query = query.redacted_only();
    }

    query = query.limit(args.limit);

    // Parse time filters
    if let Some(since_str) = &args.since {
        let since = chrono::NaiveDate::parse_from_str(since_str, "%Y-%m-%d")
            .map_err(|_| Error::InvalidTimestamp {
                timestamp: since_str.clone(),
            })?
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc();

        let end = if let Some(before_str) = &args.before {
            chrono::NaiveDate::parse_from_str(before_str, "%Y-%m-%d")
                .map_err(|_| Error::InvalidTimestamp {
                    timestamp: before_str.clone(),
                })?
                .and_hms_opt(23, 59, 59)
                .unwrap()
                .and_utc()
        } else {
            chrono::Utc::now()
        };

        query = query.with_time_range(since, end);
    }

    // Perform search
    let results = app.search_engine.search_with_query(&entries, &query)?;

    if results.is_empty() {
        if !app.quiet {
            println!("No results found for '{}'", args.term);
        }
        return Ok(());
    }

    // Display results
    for (i, result) in results.iter().enumerate() {
        if i >= args.limit {
            break;
        }

        let mut output = String::new();

        if args.timestamps {
            output.push_str(&format!("{} ", result.entry.formatted_timestamp()));
        }

        if args.show_dirs {
            output.push_str(&format!("{} ", result.entry.directory));
        }

        if let Some(ref highlighted) = result.highlighted {
            output.push_str(highlighted);
        } else {
            output.push_str(&result.entry.command);
        }

        println!("{}", output);
    }

    if !app.quiet {
        println!("\nFound {} results", results.len());
    }

    Ok(())
}

pub fn handle_recent(app: &mut CliApp, args: &RecentArgs) -> Result<()> {
    let entries = app.provider().get_recent(args.count)?;

    for entry in entries {
        if args.timestamps {
            println!("{} {}", entry.formatted_timestamp(), entry.command);
        } else {
            println!("{}", entry.command);
        }
    }

    Ok(())
}

pub fn handle_fzf(app: &mut CliApp, args: &FzfArgs) -> Result<()> {
    let mut entries = app.provider().get_entries()?;

    // Filter by directory if specified
    if let Some(dir) = &args.directory {
        entries.retain(|entry| entry.directory.contains(dir));
    }

    // Handle unique flag
    if args.unique {
        let mut seen = std::collections::HashSet::new();
        entries.retain(|entry| seen.insert(entry.command.clone()));
    }

    // Sort by timestamp
    if args.reverse {
        entries.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
    } else {
        entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    }

    // Take the requested number of entries
    entries.truncate(args.limit);

    for entry in entries {
        println!("{}", entry.command);
    }

    Ok(())
}

pub fn handle_frequent(app: &mut CliApp, args: &FrequentArgs) -> Result<()> {
    let entries = app.provider().get_entries()?;

    if args.directories {
        let frequent_dirs = app.search_engine.get_frequent_directories(&entries)?;
        for (dir, count) in frequent_dirs.iter().take(args.count) {
            if args.counts {
                println!("{}: {}", dir, count);
            } else {
                println!("{}", dir);
            }
        }
    } else {
        let frequent_commands = app.search_engine.get_frequent_commands(&entries)?;
        for (command, count) in frequent_commands.iter().take(args.count) {
            if args.counts {
                println!("{}: {}", command, count);
            } else {
                println!("{}", command);
            }
        }
    }

    Ok(())
}

pub fn handle_validate(_app: &mut CliApp, args: &ValidateArgs) -> Result<()> {
    use regex::Regex;

    // Try to compile the pattern
    match Regex::new(&args.pattern) {
        Ok(re) => {
            println!("✓ Pattern is valid: {}", args.pattern);

            if let Some(test_str) = &args.test {
                if re.is_match(test_str) {
                    println!("✓ Pattern matches test string");

                    if let Some(caps) = re.captures(test_str) {
                        println!("\nCapture groups:");
                        for (i, cap) in caps.iter().enumerate() {
                            if let Some(m) = cap {
                                println!("  Group {}: {}", i, m.as_str());
                            }
                        }
                    }
                } else {
                    println!("✗ Pattern does not match test string");
                }
            } else {
                println!("\nTo test this pattern against a string, use:");
                println!(
                    "  mortimer validate '{}' --test 'your test string'",
                    args.pattern
                );
            }
        }
        Err(e) => {
            println!("✗ Invalid pattern: {}", e);
            return Err(Error::custom(format!("Invalid regex pattern: {}", e)));
        }
    }

    Ok(())
}
