//! Alias management handlers for Mortimer CLI

use crate::cli::args::*;
use crate::cli::{CliApp, HistoryBackend};
use crate::error::{Error, Result};
use std::io::{self, BufRead};

pub fn handle_alias(app: &mut CliApp, args: &AliasArgs) -> Result<()> {
    let db = match &app.backend {
        HistoryBackend::Database(mgr) => &mgr.db,
        HistoryBackend::File(_) => {
            return Err(Error::custom(
                "Alias management requires the database backend. Remove --use-file flag to use the default database backend.",
            ));
        }
    };

    match &args.command {
        AliasCommands::Add(add_args) => {
            db.add_alias(&add_args.name, &add_args.command, &add_args.description)?;
            if !app.quiet {
                println!("Alias '{}' added successfully", add_args.name);
            }
        }
        AliasCommands::Update(update_args) => {
            db.update_alias(
                &update_args.name,
                &update_args.command,
                update_args.description.as_deref(),
            )?;
            if !app.quiet {
                println!("Alias '{}' updated successfully", update_args.name);
            }
        }
        AliasCommands::Remove(remove_args) => {
            db.remove_alias(&remove_args.name)?;
            if !app.quiet {
                println!("Alias '{}' removed successfully", remove_args.name);
            }
        }
        AliasCommands::List(list_args) => {
            let aliases = db.list_aliases()?;

            if list_args.shell {
                for a in &aliases {
                    println!("alias {}='{}'", a.alias, a.command.replace('\'', "'\\''"));
                }
            } else if aliases.is_empty() {
                println!("No aliases found.");
            } else {
                // Calculate column widths
                let name_width = aliases
                    .iter()
                    .map(|a| a.alias.len())
                    .max()
                    .unwrap_or(5)
                    .max(5);
                let cmd_width = aliases
                    .iter()
                    .map(|a| a.command.len())
                    .max()
                    .unwrap_or(7)
                    .clamp(7, 60);

                let desc_header = "DESCRIPTION";
                println!(
                    "{:<name_width$}  {:<cmd_width$}  {desc_header}",
                    "ALIAS", "COMMAND"
                );
                println!(
                    "{:<name_width$}  {:<cmd_width$}  {}",
                    "-".repeat(name_width),
                    "-".repeat(cmd_width),
                    "-".repeat(20)
                );

                for a in &aliases {
                    let cmd_display = if a.command.len() > 60 {
                        format!("{}...", &a.command[..57])
                    } else {
                        a.command.clone()
                    };
                    println!(
                        "{:<name_width$}  {:<cmd_width$}  {}",
                        a.alias, cmd_display, a.description
                    );
                }
            }
        }
        AliasCommands::Export(export_args) => {
            let aliases = db.list_aliases()?;
            let mut output = String::from("#!/bin/sh\n# Aliases exported by mortimer\n\n");

            for a in &aliases {
                output.push_str(&format!(
                    "alias {}='{}'\n",
                    a.alias,
                    a.command.replace('\'', "'\\''")
                ));
            }

            if let Some(path) = &export_args.output {
                std::fs::write(path, &output)?;
                if !app.quiet {
                    println!("Aliases exported to {}", path.display());
                }
            } else {
                print!("{}", output);
            }
        }
        AliasCommands::Sync => {
            let aliases = parse_alias_lines_from_stdin()?;
            let count = db.sync_aliases(&aliases)?;
            if !app.quiet {
                println!("Synced {} aliases", count);
            }
        }
    }

    Ok(())
}

/// Parse alias definitions from stdin (output of shell `alias` builtin)
/// Handles formats:
///   - zsh/bash: `name='command'` or `name=command`
///   - also handles: `alias name='command'`
fn parse_alias_lines_from_stdin() -> Result<Vec<(String, String)>> {
    let stdin = io::stdin();
    let mut aliases = Vec::new();

    for line in stdin.lock().lines() {
        let line = line.map_err(|e| Error::custom(format!("Failed to read stdin: {}", e)))?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Strip leading "alias " if present
        let line = line.strip_prefix("alias ").unwrap_or(line);

        // Split on first '='
        if let Some(eq_pos) = line.find('=') {
            let name = line[..eq_pos].trim().to_string();
            let mut value = line[eq_pos + 1..].trim().to_string();

            // Strip surrounding quotes
            if (value.starts_with('\'') && value.ends_with('\''))
                || (value.starts_with('"') && value.ends_with('"'))
            {
                value = value[1..value.len() - 1].to_string();
            }

            if !name.is_empty() && !value.is_empty() {
                aliases.push((name, value));
            }
        }
    }

    Ok(aliases)
}
