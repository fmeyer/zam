//! TUI handler for zam CLI

use crate::cli::{CliApp, HistoryBackend};
use crate::error::{Error, Result};
use crate::tui;
use std::env;

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

    if let Some(cmd) = tui::run_tui(&mgr.db, cwd)? {
        println!("{cmd}");
    }
    Ok(())
}
