//! TUI handler for zam CLI

use crate::cli::{CliApp, HistoryBackend};
use crate::error::{Error, Result};
use crate::tui;
use std::env;
use std::io::Write;

/// Exit status telling the shell widget to place the printed command on the
/// prompt for editing instead of running it.
pub const EXIT_EDIT: i32 = 3;

pub fn handle_tui(app: &mut CliApp) -> Result<()> {
    let mgr = match &app.backend {
        HistoryBackend::Database(mgr) => mgr,
        HistoryBackend::File(_) => {
            return Err(Error::custom(
                "TUI requires the database backend. Remove --use-file flag to use the default database backend.",
            ));
        }
    };

    let cwd = env::current_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();

    match tui::run_tui(&mgr.db, cwd)? {
        Some(tui::Selection::Execute(cmd)) => println!("{cmd}"),
        Some(tui::Selection::Edit(cmd)) => {
            println!("{cmd}");
            std::io::stdout().flush()?;
            std::process::exit(EXIT_EDIT);
        }
        None => {}
    }
    Ok(())
}
