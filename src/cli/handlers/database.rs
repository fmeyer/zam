//! Database-specific handlers for Mortimer CLI

use crate::cli::args::*;
use crate::cli::{CliApp, HistoryBackend};
use crate::error::{Error, Result};

pub fn handle_migrate(app: &mut CliApp, args: &MigrateArgs) -> Result<()> {
    let mgr = match &mut app.backend {
        HistoryBackend::Database(mgr) => mgr,
        HistoryBackend::File(_) => {
            return Err(Error::custom(
                "Migration requires database backend. Use --use-db flag.",
            ));
        }
    };

    if !app.quiet {
        println!("Migrating from .mhist file: {}", args.mhist_file.display());
    }

    if args.dry_run {
        println!("DRY RUN: Would migrate from {}", args.mhist_file.display());
        return Ok(());
    }

    let count = mgr.import_from_mhist(&args.mhist_file)?;

    if !app.quiet {
        println!("Successfully migrated {} commands", count);
    }

    Ok(())
}

pub fn handle_merge(app: &mut CliApp, args: &MergeArgs) -> Result<()> {
    let mgr = match &mut app.backend {
        HistoryBackend::Database(mgr) => mgr,
        HistoryBackend::File(_) => {
            return Err(Error::custom(
                "Merge requires database backend. Use --use-db flag.",
            ));
        }
    };

    if !app.quiet {
        println!("Merging database from: {}", args.db_file.display());
    }

    if args.dry_run {
        println!("DRY RUN: Would merge from {}", args.db_file.display());
        return Ok(());
    }

    let count = mgr.merge_from_database(&args.db_file)?;

    if !app.quiet {
        println!("Successfully merged {} commands", count);
    }

    Ok(())
}

pub fn handle_tokens(app: &mut CliApp, args: &TokensArgs) -> Result<()> {
    let mgr = match &mut app.backend {
        HistoryBackend::Database(mgr) => mgr,
        HistoryBackend::File(_) => {
            return Err(Error::custom(
                "Token management requires database backend. Use --use-db flag.",
            ));
        }
    };

    let tokens = if let Some(cmd_id) = args.command_id {
        mgr.get_tokens_for_command(cmd_id)?
    } else if let Some(ref session) = args.session {
        mgr.get_tokens_by_session(session)?
    } else if let Some(ref dir) = args.directory {
        mgr.get_tokens_by_directory(dir)?
    } else {
        return Err(Error::invalid_arguments(
            "Must specify --command-id, --session, or --directory",
        ));
    };

    if tokens.is_empty() {
        if !app.quiet {
            println!("No tokens found");
        }
        return Ok(());
    }

    println!("=== Stored Tokens ===\n");
    for token in &tokens {
        println!("ID: {}", token.id);
        println!("Command ID: {}", token.command_id);
        println!("Type: {}", token.token_type);
        println!("Placeholder: {}", token.placeholder);
        if args.show_values {
            println!("Value: {}", token.original_value);
        } else {
            println!("Value: <hidden>");
        }
        println!("Created: {}", token.created_at.format("%Y-%m-%d %H:%M:%S"));
        println!();
    }

    if !app.quiet {
        println!("Total tokens: {}", tokens.len());
    }

    Ok(())
}

pub fn handle_hosts(app: &mut CliApp, args: &HostsArgs) -> Result<()> {
    let mgr = match &app.backend {
        HistoryBackend::Database(mgr) => mgr,
        HistoryBackend::File(_) => {
            return Err(Error::custom(
                "Host management requires database backend. Use --use-db flag.",
            ));
        }
    };

    if let Some(host_id) = args.show_sessions {
        let sessions = mgr.get_sessions_for_host(host_id)?;
        println!("=== Sessions for Host ID {} ===\n", host_id);
        for session in sessions {
            println!("Session ID: {}", session.id);
            println!(
                "Started: {}",
                session.started_at.format("%Y-%m-%d %H:%M:%S")
            );
            if let Some(ended) = session.ended_at {
                println!("Ended: {}", ended.format("%Y-%m-%d %H:%M:%S"));
            } else {
                println!("Ended: <active>");
            }
            println!();
        }
    } else {
        let hosts = mgr.get_hosts()?;
        println!("=== Hosts ===\n");
        for host in hosts {
            println!("ID: {}", host.id);
            println!("Hostname: {}", host.hostname);
            println!("Created: {}", host.created_at.format("%Y-%m-%d %H:%M:%S"));
            println!();
        }
    }

    Ok(())
}

pub fn handle_sessions(app: &mut CliApp, args: &SessionsArgs) -> Result<()> {
    let mgr = match &app.backend {
        HistoryBackend::Database(mgr) => mgr,
        HistoryBackend::File(_) => {
            return Err(Error::custom(
                "Session management requires database backend. Use --use-db flag.",
            ));
        }
    };

    if let Some(host_id) = args.host_id {
        let sessions = mgr.get_sessions_for_host(host_id)?;

        let filtered: Vec<_> = if args.active {
            sessions
                .into_iter()
                .filter(|s| s.ended_at.is_none())
                .collect()
        } else {
            sessions
        };

        println!("=== Sessions ===\n");
        for session in filtered {
            println!("ID: {}", session.id);
            println!("Host ID: {}", session.host_id);
            println!(
                "Started: {}",
                session.started_at.format("%Y-%m-%d %H:%M:%S")
            );
            if let Some(ended) = session.ended_at {
                println!("Ended: {}", ended.format("%Y-%m-%d %H:%M:%S"));
            } else {
                println!("Status: Active");
            }
            println!();
        }
    } else {
        println!("Must specify --host-id");
    }

    Ok(())
}
