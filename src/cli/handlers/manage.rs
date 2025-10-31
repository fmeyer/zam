//! Management handler for interactive TUI

use crate::cli::CliApp;
use crate::error::Result;
use crate::manage_tui;

pub fn handle_manage(app: &mut CliApp) -> Result<()> {
    // Get all entries
    let entries = app.provider().get_entries()?;

    if entries.is_empty() {
        println!("No history entries to manage");
        return Ok(());
    }

    // Run the management UI
    let to_delete = manage_tui::run_management_ui(entries)?;

    if to_delete.is_empty() {
        if !app.quiet {
            println!("No entries deleted");
        }
        return Ok(());
    }

    // Delete the selected entries
    let deleted = app.provider_mut().delete_entries(&to_delete)?;

    if !app.quiet {
        println!("Successfully deleted {} entries", deleted);
    }

    Ok(())
}
