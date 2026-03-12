//! TUI handler for zam CLI

use crate::cli::{CliApp, HistoryBackend};
use crate::error::{Error, Result};
use crate::tui;

pub fn handle_tui(app: &mut CliApp) -> Result<()> {
    let mgr = match &app.backend {
        HistoryBackend::Database(mgr) => mgr,
        HistoryBackend::File(_) => {
            return Err(Error::custom(
                "TUI requires the database backend. Remove --use-file flag to use the default database backend.",
            ));
        }
    };

    tui::run_tui(&mgr.db)
}
