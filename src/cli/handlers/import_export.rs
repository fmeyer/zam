//! Import and export handlers for Mortimer CLI

use crate::cli::args::*;
use crate::cli::{CliApp, HistoryBackend};
use crate::error::Result;

pub fn handle_import(app: &mut CliApp, args: &ImportArgs) -> Result<()> {
    let shell_name = match args.shell {
        ShellType::Zsh => "zsh",
        ShellType::Bash => "bash",
        ShellType::Fish => "fish",
    };

    if !app.quiet {
        println!("Importing {} history...", shell_name);
    }

    if args.dry_run {
        println!("DRY RUN: Would import from {} history", shell_name);
        return Ok(());
    }

    let imported_count = match &mut app.backend {
        HistoryBackend::File(mgr) => mgr.import_from_shell(shell_name, args.file.clone())?,
        HistoryBackend::Database(mgr) => match args.shell {
            ShellType::Zsh => mgr.import_from_zsh(args.file.clone())?,
            ShellType::Bash => mgr.import_from_bash(args.file.clone())?,
            ShellType::Fish => mgr.import_from_fish(args.file.clone())?,
        },
    };

    if !app.quiet {
        println!(
            "Successfully imported {} commands from {} history",
            imported_count, shell_name
        );
    }

    Ok(())
}

pub fn handle_export(app: &mut CliApp, args: &ExportArgs) -> Result<()> {
    let entries = match &app.backend {
        HistoryBackend::File(mgr) => mgr.get_entries()?,
        HistoryBackend::Database(mgr) => mgr
            .get_all_commands()?
            .into_iter()
            .map(Into::into)
            .collect(),
    };

    // Filter entries if needed
    let filtered_entries: Vec<_> = entries
        .into_iter()
        .filter(|entry| {
            if let Some(dir) = &args.directory {
                if !entry.directory.contains(dir) {
                    return false;
                }
            }

            if let Some(days) = args.days {
                let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);
                if entry.timestamp < cutoff {
                    return false;
                }
            }

            true
        })
        .collect();

    let output = match args.format {
        ExportFormat::Json => serde_json::to_string_pretty(&filtered_entries)?,
        ExportFormat::Csv => {
            let mut output = String::from("timestamp,directory,command\n");
            for entry in &filtered_entries {
                output.push_str(&format!(
                    "{},{},{}\n",
                    entry.timestamp.to_rfc3339(),
                    entry.directory,
                    entry.command.replace(",", "\\,")
                ));
            }
            output
        }
        ExportFormat::Tsv => {
            let mut output = String::from("timestamp\tdirectory\tcommand\n");
            for entry in &filtered_entries {
                output.push_str(&format!(
                    "{}\t{}\t{}\n",
                    entry.timestamp.to_rfc3339(),
                    entry.directory,
                    entry.command
                ));
            }
            output
        }
        ExportFormat::Plain => {
            let mut output = String::new();
            for entry in &filtered_entries {
                output.push_str(&entry.command);
                output.push('\n');
            }
            output
        }
    };

    if let Some(output_file) = &args.output {
        std::fs::write(output_file, output)?;
        if !app.quiet {
            println!(
                "Exported {} entries to {}",
                filtered_entries.len(),
                output_file.display()
            );
        }
    } else {
        print!("{}", output);
    }

    Ok(())
}
