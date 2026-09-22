//! Interactive TUI for browsing and managing all database entities

use crate::database::{Alias, CommandEntry, Database, Host, MatchMode, Session, Token};
use crate::error::Result;
use crossterm::{
    event::{
        self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, KeyboardEnhancementFlags,
        PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState, Wrap},
};
use std::fs::File;

/// Color theme for the TUI, with dark and light variants.
/// Palette derived from Matrix and Photophobia Zed themes.
struct Theme {
    tab_number: Color,
    tab_text: Color,
    tab_highlight: Color,
    header: Color,
    row_highlight: Color,
    status_default: Color,
    status_active: Color,
    popup_text: Color,
    popup_confirm: Color,
    popup_accent: Color,
    match_highlight: Color,
    success: Color,
    error: Color,
    flash_bg: Color,
    flash_fg: Color,
}

impl Theme {
    fn dark() -> Self {
        Self {
            tab_number: Color::Rgb(0x58, 0x58, 0x58), // matrix dark text.placeholder
            tab_text: Color::Rgb(0xbc, 0xbc, 0xbc),   // matrix dark text
            tab_highlight: Color::Rgb(0x5f, 0xb4, 0xb4), // matrix dark accent teal
            header: Color::Rgb(0x4a, 0x98, 0x98),     // matrix dark comment/dim teal
            row_highlight: Color::Rgb(0x30, 0x35, 0x40), // matrix dark element.selected
            status_default: Color::Rgb(0x58, 0x58, 0x58), // matrix dark text.placeholder
            status_active: Color::Rgb(0x5f, 0xb4, 0xb4), // matrix dark accent teal
            popup_text: Color::Rgb(0xbc, 0xbc, 0xbc), // matrix dark text
            popup_confirm: Color::Rgb(0xc6, 0x95, 0xc6), // matrix dark magenta
            popup_accent: Color::Rgb(0x5f, 0xb4, 0xb4), // matrix dark accent teal
            match_highlight: Color::Rgb(0x5f, 0xb4, 0xb4), // matrix dark accent teal
            success: Color::Rgb(0x99, 0xc7, 0x94),    // matrix dark green
            error: Color::Rgb(0xc6, 0x95, 0xc6),      // matrix dark magenta
            flash_bg: Color::Rgb(0x99, 0xc7, 0x94),   // matrix dark green
            flash_fg: Color::Rgb(0x15, 0x19, 0x1e),   // matrix dark background
        }
    }

    fn light() -> Self {
        Self {
            tab_number: Color::Rgb(0x99, 0x99, 0x57), // photophobia line_number
            tab_text: Color::Rgb(0x42, 0x42, 0x42),   // photophobia foreground
            tab_highlight: Color::Rgb(0x2a, 0x8d, 0xc5), // photophobia blue
            header: Color::Rgb(0x99, 0x99, 0x57),     // photophobia line_number
            row_highlight: Color::Rgb(0xc7, 0xde, 0xff), // photophobia element.selected
            status_default: Color::Rgb(0x99, 0x99, 0x57), // photophobia line_number
            status_active: Color::Rgb(0x2a, 0x8d, 0xc5), // photophobia blue
            popup_text: Color::Rgb(0x42, 0x42, 0x42), // photophobia foreground
            popup_confirm: Color::Rgb(0xb8, 0x5c, 0x57), // photophobia red
            popup_accent: Color::Rgb(0x2a, 0x8d, 0xc5), // photophobia blue
            match_highlight: Color::Rgb(0x1a, 0x3a, 0x6b), // photophobia comment blue
            success: Color::Rgb(0x57, 0x86, 0x4e),    // photophobia green
            error: Color::Rgb(0xb8, 0x5c, 0x57),      // photophobia red
            flash_bg: Color::Rgb(0x57, 0x86, 0x4e),   // photophobia green
            flash_fg: Color::Rgb(0xf9, 0xf9, 0xee),   // photophobia background
        }
    }

    fn detect() -> Self {
        // ZAM_THEME env var overrides auto-detection
        if let Ok(val) = std::env::var("ZAM_THEME") {
            return match val.to_lowercase().as_str() {
                "light" | "white" => Self::light(),
                _ => Self::dark(),
            };
        }

        if Self::system_is_light() {
            Self::light()
        } else {
            Self::dark()
        }
    }

    #[cfg(target_os = "macos")]
    fn system_is_light() -> bool {
        std::process::Command::new("defaults")
            .args(["read", "-g", "AppleInterfaceStyle"])
            .output()
            .map(|o| !o.status.success()) // exits non-zero when light mode (no key set)
            .unwrap_or(false)
    }

    #[cfg(not(target_os = "macos"))]
    fn system_is_light() -> bool {
        // COLORFGBG="15;0" means light-on-dark, "0;15" means dark-on-light
        std::env::var("COLORFGBG")
            .map(|v| {
                v.rsplit(';')
                    .next()
                    .and_then(|bg| bg.parse::<u8>().ok())
                    .is_some_and(|bg| bg > 8)
            })
            .unwrap_or(false)
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Local,
    Frequent,
    Commands,
    Aliases,
    Hosts,
    Sessions,
    Tokens,
    Help,
}

const TABS: [Tab; 8] = [
    Tab::Commands,
    Tab::Local,
    Tab::Sessions,
    Tab::Frequent,
    Tab::Aliases,
    Tab::Hosts,
    Tab::Tokens,
    Tab::Help,
];

impl Tab {
    fn title(self) -> &'static str {
        match self {
            Tab::Local => "local",
            Tab::Frequent => "top 50",
            Tab::Commands => "global",
            Tab::Aliases => "aliases",
            Tab::Hosts => "hosts",
            Tab::Sessions => "sessions",
            Tab::Tokens => "tokens",
            Tab::Help => "?",
        }
    }

    fn index(self) -> usize {
        TABS.iter().position(|&t| t == self).unwrap_or(0)
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Filter,
    Confirm,
    EditAlias,
}

#[derive(Clone, Copy, PartialEq)]
enum EditField {
    Command,
    Description,
}

struct FrequentCommand {
    command: String,
    count: usize,
}

struct AppTUI<'a> {
    db: &'a Database,
    cwd: String,
    home: String,
    tab: Tab,
    mode: Mode,
    theme: Theme,

    // Data
    commands: Vec<CommandEntry>,
    local_commands: Vec<CommandEntry>,
    frequent: Vec<FrequentCommand>,
    aliases: Vec<Alias>,
    hosts: Vec<Host>,
    sessions: Vec<Session>,
    session_cmd_counts: Vec<usize>,
    tokens: Vec<Token>,

    // Session detail drill-down
    session_commands: Vec<CommandEntry>,
    session_detail_id: Option<String>,

    // Pagination (Commands and Sessions tabs)
    page: usize,
    page_size: usize,
    total_paged_rows: usize,

    // Table state per tab
    table_state: TableState,
    row_count: usize,

    // Filter
    filter: String,
    match_mode: MatchMode,

    // Confirm delete
    confirm_msg: String,

    // Edit alias
    edit_field: EditField,
    edit_buf: String,
    edit_alias_name: String,

    // Status
    copied_at: Option<std::time::Instant>,
    status: Option<String>,
    show_values: bool,
    relative_time: bool,
    running: bool,
    selected_command: Option<String>,
    /// Whether the selected command should be executed (Enter) or only
    /// placed on the command line for editing (Shift+Enter / Alt+Enter).
    execute_selected: bool,
}

impl<'a> AppTUI<'a> {
    fn new(db: &'a Database, cwd: String) -> Result<Self> {
        let mut app = Self {
            db,
            cwd,
            home: std::env::var("HOME").unwrap_or_default(),
            tab: Tab::Commands,
            mode: Mode::Filter,
            theme: Theme::detect(),
            commands: Vec::new(),
            local_commands: Vec::new(),
            session_commands: Vec::new(),
            session_detail_id: None,
            frequent: Vec::new(),
            aliases: Vec::new(),
            hosts: Vec::new(),
            sessions: Vec::new(),
            session_cmd_counts: Vec::new(),
            tokens: Vec::new(),
            page: 0,
            page_size: 100,
            total_paged_rows: 0,
            table_state: TableState::default(),
            row_count: 0,
            filter: String::new(),
            match_mode: if db.get_bool_preference("fuzzy_search").unwrap_or(true) {
                MatchMode::Fuzzy
            } else {
                MatchMode::Substring
            },
            confirm_msg: String::new(),
            edit_field: EditField::Command,
            edit_buf: String::new(),
            edit_alias_name: String::new(),
            copied_at: None,
            status: None,
            show_values: false,
            relative_time: db.get_bool_preference("relative_time").unwrap_or(false),
            running: true,
            selected_command: None,
            execute_selected: true,
        };
        // Restore last tab from preferences
        if let Ok(Some(val)) = db.get_preference("last_tab")
            && let Ok(idx) = val.parse::<usize>()
            && idx < TABS.len()
        {
            app.tab = TABS[idx];
        }
        app.load_tab()?;
        Ok(app)
    }

    fn load_tab(&mut self) -> Result<()> {
        self.table_state = TableState::default();
        let filter = if self.filter.is_empty() {
            None
        } else {
            Some(self.filter.as_str())
        };
        match self.tab {
            Tab::Commands => {
                self.total_paged_rows = self
                    .db
                    .count_unique_commands_filtered(filter, self.match_mode)?;
                self.commands = self.db.get_unique_commands_filtered(
                    self.page * self.page_size,
                    self.page_size,
                    filter,
                    self.match_mode,
                )?;
                self.row_count = self.commands.len();
            }
            Tab::Sessions => {
                self.total_paged_rows = self.db.count_sessions_filtered(filter, self.match_mode)?;
                self.sessions = self.db.get_sessions_filtered(
                    self.page * self.page_size,
                    self.page_size,
                    filter,
                    self.match_mode,
                )?;
                let sids: Vec<&str> = self.sessions.iter().map(|s| s.id.as_ref()).collect();
                self.session_cmd_counts = self.db.count_commands_for_sessions(&sids)?;
                self.row_count = self.sessions.len();
            }
            Tab::Local => {
                self.local_commands = self.db.get_commands_for_directory(&self.cwd)?;
                self.row_count = self.local_commands.len();
            }
            Tab::Frequent => {
                self.frequent = self
                    .db
                    .get_frequent_commands(50)?
                    .into_iter()
                    .map(|(command, count)| FrequentCommand { command, count })
                    .collect();
                self.row_count = self.frequent.len();
            }
            Tab::Aliases => {
                self.aliases = self.db.list_aliases()?;
                self.row_count = self.aliases.len();
            }
            Tab::Hosts => {
                self.hosts = self.db.get_hosts()?;
                self.row_count = self.hosts.len();
            }
            Tab::Tokens => {
                self.tokens = self.db.get_all_tokens()?;
                self.row_count = self.tokens.len();
            }
            Tab::Help => {
                self.row_count = 0;
            }
        }
        if self.row_count > 0 {
            self.table_state.select(Some(0));
        }
        Ok(())
    }

    /// Map the selected table row back to the original data index,
    /// accounting for filter. For DB-filtered tabs (Commands, Sessions)
    /// the loaded data is already filtered, so index maps directly.
    /// For other tabs, client-side filter maps the Nth visible row to its data index.
    fn resolve_selected(&self) -> Option<usize> {
        let sel = self.table_state.selected()?;
        // DB-filtered tabs: data is already filtered
        if matches!(self.tab, Tab::Commands) {
            return Some(sel);
        }
        if matches!(self.tab, Tab::Sessions) && self.session_detail_id.is_none() {
            return Some(sel);
        }
        if self.filter.is_empty() {
            return Some(sel);
        }
        // Client-side filtered tabs: find the sel-th matching item
        let matching_indices: Vec<usize> = match self.tab {
            Tab::Local => self
                .local_commands
                .iter()
                .enumerate()
                .filter(|(_, c)| self.matches_filter(&c.command))
                .map(|(i, _)| i)
                .collect(),
            Tab::Frequent => self
                .frequent
                .iter()
                .enumerate()
                .filter(|(_, f)| self.matches_filter(&f.command))
                .map(|(i, _)| i)
                .collect(),
            Tab::Aliases => self
                .aliases
                .iter()
                .enumerate()
                .filter(|(_, a)| self.matches_filter(&a.alias) || self.matches_filter(&a.command))
                .map(|(i, _)| i)
                .collect(),
            Tab::Hosts => self
                .hosts
                .iter()
                .enumerate()
                .filter(|(_, h)| self.matches_filter(&h.hostname))
                .map(|(i, _)| i)
                .collect(),
            Tab::Sessions => self
                .session_commands
                .iter()
                .enumerate()
                .filter(|(_, c)| self.matches_filter(&c.command))
                .map(|(i, _)| i)
                .collect(),
            Tab::Tokens => self
                .tokens
                .iter()
                .enumerate()
                .filter(|(_, t)| {
                    self.matches_filter(&t.token_type) || self.matches_filter(&t.placeholder)
                })
                .map(|(i, _)| i)
                .collect(),
            _ => return Some(sel),
        };
        matching_indices.get(sel).copied()
    }

    /// Count of rows currently visible (after filtering).
    /// For DB-filtered tabs, row_count already reflects the filter.
    fn filtered_row_count(&self) -> usize {
        // DB-filtered tabs
        if matches!(self.tab, Tab::Commands) {
            return self.row_count;
        }
        if matches!(self.tab, Tab::Sessions) && self.session_detail_id.is_none() {
            return self.row_count;
        }
        if self.filter.is_empty() {
            return self.row_count;
        }
        match self.tab {
            Tab::Local => self
                .local_commands
                .iter()
                .filter(|c| self.matches_filter(&c.command))
                .count(),
            Tab::Frequent => self
                .frequent
                .iter()
                .filter(|f| self.matches_filter(&f.command))
                .count(),
            Tab::Aliases => self
                .aliases
                .iter()
                .filter(|a| self.matches_filter(&a.alias) || self.matches_filter(&a.command))
                .count(),
            Tab::Hosts => self
                .hosts
                .iter()
                .filter(|h| self.matches_filter(&h.hostname))
                .count(),
            Tab::Sessions => self
                .session_commands
                .iter()
                .filter(|c| self.matches_filter(&c.command))
                .count(),
            Tab::Tokens => self
                .tokens
                .iter()
                .filter(|t| {
                    self.matches_filter(&t.token_type) || self.matches_filter(&t.placeholder)
                })
                .count(),
            // Commands tab is DB-filtered, handled by early return above
            Tab::Commands => self.row_count,
            Tab::Help => 0,
        }
    }

    fn select_prev(&mut self) {
        let count = self.filtered_row_count();
        if count == 0 {
            return;
        }
        let i = self
            .table_state
            .selected()
            .map(|s| s.saturating_sub(1))
            .unwrap_or(0);
        self.table_state.select(Some(i));
    }

    fn select_next(&mut self) {
        let count = self.filtered_row_count();
        if count == 0 {
            return;
        }
        let i = self
            .table_state
            .selected()
            .map(|s| (s + 1).min(count - 1))
            .unwrap_or(0);
        self.table_state.select(Some(i));
    }

    fn next_tab(&mut self) -> Result<()> {
        let idx = (self.tab.index() + 1) % TABS.len();
        self.tab = TABS[idx];
        self.filter.clear();
        self.page = 0;
        self.session_detail_id = None;
        self.session_commands.clear();
        let _ = self.db.set_preference("last_tab", &idx.to_string());
        self.load_tab()
    }

    fn prev_tab(&mut self) -> Result<()> {
        let idx = if self.tab.index() == 0 {
            TABS.len() - 1
        } else {
            self.tab.index() - 1
        };
        self.tab = TABS[idx];
        self.filter.clear();
        self.page = 0;
        self.session_detail_id = None;
        self.session_commands.clear();
        let _ = self.db.set_preference("last_tab", &idx.to_string());
        self.load_tab()
    }

    fn request_delete(&mut self) {
        let Some(idx) = self.resolve_selected() else {
            return;
        };
        let msg = match self.tab {
            Tab::Commands => {
                if let Some(cmd) = self.commands.get(idx) {
                    let preview: String = cmd.command.chars().take(40).collect();
                    format!("Delete entry \"{}\"?", preview)
                } else {
                    return;
                }
            }
            Tab::Local => {
                if let Some(cmd) = self.local_commands.get(idx) {
                    let preview: String = cmd.command.chars().take(40).collect();
                    format!("Delete entry \"{}\"?", preview)
                } else {
                    return;
                }
            }
            Tab::Aliases => {
                if let Some(a) = self.aliases.get(idx) {
                    format!("Delete alias '{}'?", a.alias)
                } else {
                    return;
                }
            }
            Tab::Hosts => {
                if let Some(h) = self.hosts.get(idx) {
                    format!(
                        "Delete host '{}' and all its sessions/commands?",
                        h.hostname
                    )
                } else {
                    return;
                }
            }
            Tab::Sessions => {
                if let Some(s) = self.sessions.get(idx) {
                    format!("Delete session {} and all its commands?", s.id)
                } else {
                    return;
                }
            }
            Tab::Tokens => {
                if let Some(t) = self.tokens.get(idx) {
                    format!("Delete token {} ({})?", t.id, t.token_type)
                } else {
                    return;
                }
            }
            Tab::Frequent | Tab::Help => return,
        };
        self.confirm_msg = msg;
        self.mode = Mode::Confirm;
    }

    fn confirm_delete(&mut self) -> Result<()> {
        let Some(idx) = self.resolve_selected() else {
            self.mode = Mode::Filter;
            return Ok(());
        };
        match self.tab {
            Tab::Commands => {
                if let Some(cmd) = self.commands.get(idx) {
                    self.db.delete_command(cmd.id)?;
                    self.status = Some("Entry deleted".into());
                }
            }
            Tab::Local => {
                if let Some(cmd) = self.local_commands.get(idx) {
                    self.db.delete_command(cmd.id)?;
                    self.status = Some("Entry deleted".into());
                }
            }
            Tab::Aliases => {
                if let Some(a) = self.aliases.get(idx) {
                    self.db.remove_alias(&a.alias)?;
                    self.status = Some(format!("Alias '{}' deleted", a.alias));
                }
            }
            Tab::Hosts => {
                if let Some(h) = self.hosts.get(idx) {
                    self.db.delete_host(h.id)?;
                    self.status = Some(format!("Host '{}' deleted", h.hostname));
                }
            }
            Tab::Sessions => {
                if let Some(s) = self.sessions.get(idx) {
                    self.db.delete_session(s.id.as_ref())?;
                    self.status = Some("Session deleted".into());
                }
            }
            Tab::Tokens => {
                if let Some(t) = self.tokens.get(idx) {
                    self.db.delete_token(t.id)?;
                    self.status = Some("Token deleted".into());
                }
            }
            Tab::Frequent | Tab::Help => {}
        }
        self.mode = Mode::Filter;
        self.load_tab()
    }

    fn is_paginated_tab(&self) -> bool {
        matches!(self.tab, Tab::Commands | Tab::Sessions) && self.session_detail_id.is_none()
    }

    fn total_pages(&self) -> usize {
        if self.total_paged_rows == 0 {
            1
        } else {
            self.total_paged_rows.div_ceil(self.page_size)
        }
    }

    fn next_page(&mut self) -> Result<()> {
        if !self.is_paginated_tab() {
            return Ok(());
        }
        if self.page + 1 < self.total_pages() {
            self.page += 1;
            self.load_tab()?;
        }
        Ok(())
    }

    fn prev_page(&mut self) -> Result<()> {
        if !self.is_paginated_tab() {
            return Ok(());
        }
        if self.page > 0 {
            self.page -= 1;
            self.load_tab()?;
        }
        Ok(())
    }

    fn start_edit_alias(&mut self) {
        if self.tab != Tab::Aliases {
            return;
        }
        let Some(idx) = self.resolve_selected() else {
            return;
        };
        let Some(a) = self.aliases.get(idx) else {
            return;
        };
        self.edit_alias_name = a.alias.clone();
        self.edit_field = EditField::Command;
        self.edit_buf = a.command.clone();
        self.mode = Mode::EditAlias;
    }

    fn commit_edit_alias(&mut self) -> Result<()> {
        let value = self.edit_buf.trim().to_string();
        if value.is_empty() {
            self.mode = Mode::Filter;
            return Ok(());
        }
        match self.edit_field {
            EditField::Command => {
                self.db.update_alias(&self.edit_alias_name, &value, None)?;
                self.status = Some(format!("Alias '{}' command updated", self.edit_alias_name));
            }
            EditField::Description => {
                // Fetch current command to preserve it
                if let Some(a) = self
                    .aliases
                    .iter()
                    .find(|a| a.alias == self.edit_alias_name)
                {
                    self.db
                        .update_alias(&self.edit_alias_name, &a.command, Some(&value))?;
                    self.status = Some(format!(
                        "Alias '{}' description updated",
                        self.edit_alias_name
                    ));
                }
            }
        }
        self.mode = Mode::Filter;
        self.load_tab()
    }

    fn jump_to_session_current(&mut self) -> Result<()> {
        self.jump_to_tab(Tab::Sessions.index())?;
        // Auto-drill into the current session if ZAM_SESSION_ID is set
        if let Ok(sid) = std::env::var("ZAM_SESSION_ID") {
            self.session_commands = self.db.get_commands_for_session(&sid)?;
            self.row_count = self.session_commands.len();
            self.session_detail_id = Some(sid);
            self.filter.clear();
            self.table_state = TableState::default();
            if self.row_count > 0 {
                self.table_state.select(Some(0));
            }
        }
        Ok(())
    }

    fn jump_to_tab(&mut self, idx: usize) -> Result<()> {
        if idx < TABS.len() {
            self.tab = TABS[idx];
            self.filter.clear();
            self.page = 0;
            self.session_detail_id = None;
            self.session_commands.clear();
            let _ = self.db.set_preference("last_tab", &idx.to_string());
            self.load_tab()?;
        }
        Ok(())
    }

    /// Restore the original unredacted command by replacing placeholders with
    /// their stored token values. Returns `None` if the command is redacted but
    /// cannot be fully restored (missing tokens).
    fn unredact_command(&self, entry: &CommandEntry) -> Option<String> {
        if !entry.redacted {
            return Some(entry.command.clone());
        }
        let tokens = self.db.get_tokens_for_command(entry.id).ok()?;
        if tokens.is_empty() {
            return None;
        }
        let mut cmd = entry.command.clone();
        for token in &tokens {
            cmd = cmd.replace(&token.placeholder, &token.original_value);
        }
        Some(cmd)
    }

    /// Get the command string for the currently selected row, if applicable.
    fn selected_command_text(&self) -> Option<String> {
        let idx = self.resolve_selected()?;
        match self.tab {
            Tab::Commands => self
                .commands
                .get(idx)
                .and_then(|c| self.unredact_command(c)),
            Tab::Local => self
                .local_commands
                .get(idx)
                .and_then(|c| self.unredact_command(c)),
            Tab::Frequent => self.frequent.get(idx).map(|f| f.command.clone()),
            Tab::Sessions if self.session_detail_id.is_some() => self
                .session_commands
                .get(idx)
                .and_then(|c| self.unredact_command(c)),
            _ => None,
        }
    }

    /// Full text of the selected row for the preview pane. Shows the stored
    /// (redacted) form so secrets are never painted on screen.
    fn selected_preview_text(&self) -> Option<&str> {
        let idx = self.resolve_selected()?;
        match self.tab {
            Tab::Commands => self.commands.get(idx).map(|c| c.command.as_str()),
            Tab::Local => self.local_commands.get(idx).map(|c| c.command.as_str()),
            Tab::Frequent => self.frequent.get(idx).map(|f| f.command.as_str()),
            Tab::Aliases => self.aliases.get(idx).map(|a| a.command.as_str()),
            Tab::Sessions if self.session_detail_id.is_some() => {
                self.session_commands.get(idx).map(|c| c.command.as_str())
            }
            _ => None,
        }
    }

    /// Copy the currently selected command to the system clipboard via pbcopy.
    fn yank_to_clipboard(&mut self) {
        let Some(cmd) = self.selected_command_text() else {
            return;
        };
        use std::io::Write;
        let result = std::process::Command::new("pbcopy")
            .stdin(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                if let Some(ref mut stdin) = child.stdin {
                    stdin.write_all(cmd.as_bytes())?;
                }
                child.wait()
            });
        match result {
            Ok(status) if status.success() => {
                self.copied_at = Some(std::time::Instant::now());
            }
            _ => {
                self.status = Some("Failed to copy to clipboard".into());
            }
        }
    }

    fn toggle_match_mode(&mut self) -> Result<()> {
        self.match_mode = match self.match_mode {
            MatchMode::Fuzzy => MatchMode::Substring,
            MatchMode::Substring => MatchMode::Fuzzy,
        };
        let _ = self.db.set_preference(
            "fuzzy_search",
            if self.match_mode == MatchMode::Fuzzy {
                "true"
            } else {
                "false"
            },
        );
        self.page = 0;
        self.load_tab()
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        // Global modifier shortcuts work in any mode except Confirm and EditAlias
        if self.mode != Mode::Confirm && self.mode != Mode::EditAlias {
            // Alt+1..8 jump to tab by number
            if key.modifiers.contains(KeyModifiers::ALT)
                && let KeyCode::Char(c @ '1'..='8') = key.code
            {
                self.jump_to_tab((c as usize) - ('1' as usize))?;
                return Ok(());
            }

            // Ctrl shortcuts
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                match key.code {
                    KeyCode::Char('c') => {
                        self.running = false;
                        return Ok(());
                    }
                    KeyCode::Char('d') => {
                        self.request_delete();
                        return Ok(());
                    }
                    KeyCode::Char('e') if self.tab == Tab::Aliases => {
                        self.start_edit_alias();
                        return Ok(());
                    }
                    KeyCode::Char('t') => {
                        self.relative_time = !self.relative_time;
                        let _ = self.db.set_preference(
                            "relative_time",
                            if self.relative_time { "true" } else { "false" },
                        );
                        return Ok(());
                    }
                    KeyCode::Char('v') if self.tab == Tab::Tokens => {
                        self.show_values = !self.show_values;
                        return Ok(());
                    }
                    KeyCode::Char('l') => {
                        self.jump_to_tab(Tab::Local.index())?;
                        return Ok(());
                    }
                    KeyCode::Char('s') => {
                        self.jump_to_session_current()?;
                        return Ok(());
                    }
                    KeyCode::Char('h') => {
                        self.jump_to_tab(Tab::Commands.index())?;
                        return Ok(());
                    }
                    KeyCode::Char('y') => {
                        self.yank_to_clipboard();
                        return Ok(());
                    }
                    KeyCode::Char('f') => {
                        self.toggle_match_mode()?;
                        return Ok(());
                    }
                    _ => {}
                }
            }
        }

        if key.code == KeyCode::Enter {
            // Shift+Enter (terminals with keyboard enhancement) or Alt+Enter
            // (everywhere) places the command on the prompt without running it.
            self.execute_selected = !key
                .modifiers
                .intersects(KeyModifiers::SHIFT | KeyModifiers::ALT);
        }

        match self.mode {
            Mode::Confirm => match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => self.confirm_delete()?,
                _ => self.mode = Mode::Filter,
            },
            Mode::EditAlias => match key.code {
                KeyCode::Esc => {
                    self.mode = Mode::Filter;
                }
                KeyCode::Enter => {
                    self.commit_edit_alias()?;
                }
                KeyCode::Tab => {
                    // Switch between editing command and description
                    let Some(a) = self
                        .aliases
                        .iter()
                        .find(|a| a.alias == self.edit_alias_name)
                    else {
                        self.mode = Mode::Filter;
                        return Ok(());
                    };
                    match self.edit_field {
                        EditField::Command => {
                            self.edit_field = EditField::Description;
                            self.edit_buf = a.description.clone();
                        }
                        EditField::Description => {
                            self.edit_field = EditField::Command;
                            self.edit_buf = a.command.clone();
                        }
                    }
                }
                KeyCode::Backspace => {
                    self.edit_buf.pop();
                }
                KeyCode::Char(c) => {
                    self.edit_buf.push(c);
                }
                _ => {}
            },
            Mode::Filter => match key.code {
                KeyCode::Esc => {
                    if self.session_detail_id.is_some() {
                        self.session_detail_id = None;
                        self.session_commands.clear();
                        self.filter.clear();
                        self.page = 0;
                        self.load_tab()?;
                    } else if self.tab == Tab::Help {
                        self.jump_to_tab(Tab::Commands.index())?;
                    } else if self.filter.is_empty() {
                        self.running = false;
                    } else {
                        self.filter.clear();
                        self.page = 0;
                        self.load_tab()?;
                    }
                }
                KeyCode::Enter if self.tab == Tab::Frequent => {
                    if let Some(idx) = self.resolve_selected()
                        && let Some(f) = self.frequent.get(idx)
                    {
                        self.selected_command = Some(f.command.clone());
                        self.running = false;
                    }
                }
                KeyCode::Enter if self.tab == Tab::Local => {
                    if let Some(idx) = self.resolve_selected()
                        && let Some(cmd) = self.local_commands.get(idx)
                    {
                        if let Some(resolved) = self.unredact_command(cmd) {
                            self.selected_command = Some(resolved);
                            self.running = false;
                        } else {
                            self.status = Some(
                                "Cannot execute: redacted command has no stored tokens".into(),
                            );
                        }
                    }
                }
                KeyCode::Enter if self.tab == Tab::Sessions && self.session_detail_id.is_some() => {
                    if let Some(idx) = self.resolve_selected()
                        && let Some(cmd) = self.session_commands.get(idx)
                    {
                        if let Some(resolved) = self.unredact_command(cmd) {
                            self.selected_command = Some(resolved);
                            self.running = false;
                        } else {
                            self.status = Some(
                                "Cannot execute: redacted command has no stored tokens".into(),
                            );
                        }
                    }
                }
                KeyCode::Enter if self.tab == Tab::Sessions && self.session_detail_id.is_none() => {
                    if let Some(idx) = self.resolve_selected()
                        && let Some(s) = self.sessions.get(idx)
                    {
                        let sid = s.id.as_ref().to_string();
                        self.session_commands = self.db.get_commands_for_session(&sid)?;
                        self.row_count = self.session_commands.len();
                        self.session_detail_id = Some(sid);
                        self.filter.clear();
                        self.table_state = TableState::default();
                        if self.row_count > 0 {
                            self.table_state.select(Some(0));
                        }
                    }
                }
                KeyCode::Enter if self.tab == Tab::Commands => {
                    if let Some(idx) = self.resolve_selected()
                        && let Some(cmd) = self.commands.get(idx)
                    {
                        if let Some(resolved) = self.unredact_command(cmd) {
                            self.selected_command = Some(resolved);
                            self.running = false;
                        } else {
                            self.status = Some(
                                "Cannot execute: redacted command has no stored tokens".into(),
                            );
                        }
                    }
                }
                KeyCode::Enter => {}
                KeyCode::Backspace => {
                    self.filter.pop();
                    self.page = 0;
                    self.load_tab()?;
                }
                KeyCode::Up => self.select_prev(),
                KeyCode::Down => self.select_next(),
                KeyCode::Left if self.is_paginated_tab() => self.prev_page()?,
                KeyCode::Right if self.is_paginated_tab() => self.next_page()?,
                KeyCode::Left => self.prev_tab()?,
                KeyCode::Right => self.next_tab()?,
                KeyCode::Tab => self.next_tab()?,
                KeyCode::BackTab => self.prev_tab()?,
                KeyCode::Delete => self.request_delete(),
                KeyCode::Char('?') if self.filter.is_empty() => {
                    self.jump_to_tab(Tab::Help.index())?;
                }
                KeyCode::Char(c) => {
                    self.filter.push(c);
                    self.page = 0;
                    self.load_tab()?;
                }
                _ => {}
            },
        }
        Ok(())
    }

    fn render(&mut self, frame: &mut Frame) {
        let outer = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(1), Constraint::Min(1)])
            .split(frame.area());

        let preview_height = self.preview_height(outer[1]);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(5),                 // Table
                Constraint::Length(1),              // Tab bar
                Constraint::Length(preview_height), // Preview
                Constraint::Length(1),              // Status
            ])
            .split(outer[1]);

        self.render_table(frame, chunks[0]);
        self.render_tabs(frame, chunks[1]);
        self.render_preview(frame, chunks[2]);
        self.render_status(frame, chunks[3]);

        match self.mode {
            Mode::Confirm => self.render_confirm(frame, frame.area()),
            Mode::EditAlias => self.render_edit_alias(frame, frame.area()),
            _ => {}
        }
    }

    /// Rows needed to show the selected command in full (plus a separator
    /// line), capped to a third of the screen so the table stays usable.
    fn preview_height(&self, area: Rect) -> u16 {
        let Some(text) = self.selected_preview_text() else {
            return 0;
        };
        let width = area.width.max(1) as usize;
        let lines: usize = text
            .lines()
            .map(|l| l.chars().count().div_ceil(width).max(1))
            .sum::<usize>()
            .max(1);
        let max = (area.height / 3).max(2) as usize;
        (lines + 1).min(max) as u16
    }

    fn render_preview(&self, frame: &mut Frame, area: Rect) {
        if area.height == 0 {
            return;
        }
        let Some(text) = self.selected_preview_text() else {
            return;
        };
        let lines: Vec<Line> = text
            .lines()
            .map(|l| {
                if self.filter.is_empty() {
                    Line::from(l)
                } else {
                    highlight_matches(l, &self.filter, self.match_mode, self.theme.match_highlight)
                }
            })
            .collect();
        let p = Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::TOP)
                    .border_style(Style::default().fg(self.theme.tab_number)),
            )
            .style(Style::default().fg(self.theme.tab_text))
            .wrap(Wrap { trim: false });
        frame.render_widget(p, area);
    }

    fn render_tabs(&self, frame: &mut Frame, area: Rect) {
        let mut spans: Vec<Span> = Vec::new();
        let active_idx = self.tab.index();
        for (i, t) in TABS.iter().enumerate() {
            if *t == Tab::Help {
                continue;
            }
            if i > 0 && TABS[i - 1] != Tab::Help {
                spans.push(Span::styled(
                    " ",
                    Style::default().fg(self.theme.tab_number),
                ));
            }
            if i == active_idx {
                spans.push(Span::styled(
                    format!(" {} ", t.title()),
                    Style::default()
                        .fg(self.theme.tab_highlight)
                        .add_modifier(Modifier::BOLD | Modifier::REVERSED),
                ));
            } else {
                spans.push(Span::styled(
                    format!(" {} ", t.title()),
                    Style::default().fg(self.theme.tab_number),
                ));
            }
        }
        // Right-align help hint
        let used: usize = spans.iter().map(|s| s.width()).sum();
        let help_text = " ? help ";
        let remaining = (area.width as usize).saturating_sub(used + help_text.len());
        if remaining > 0 {
            spans.push(Span::raw(" ".repeat(remaining)));
        }
        if self.tab == Tab::Help {
            spans.push(Span::styled(
                help_text,
                Style::default()
                    .fg(self.theme.tab_highlight)
                    .add_modifier(Modifier::BOLD | Modifier::REVERSED),
            ));
        } else {
            spans.push(Span::styled(
                help_text,
                Style::default().fg(self.theme.tab_number),
            ));
        }
        let line = Line::from(spans);
        frame.render_widget(Paragraph::new(line), area);
    }

    fn render_table(&mut self, frame: &mut Frame, area: Rect) {
        match self.tab {
            Tab::Local => self.render_local(frame, area),
            Tab::Frequent => self.render_frequent(frame, area),
            Tab::Commands => self.render_commands(frame, area),
            Tab::Aliases => self.render_aliases(frame, area),
            Tab::Hosts => self.render_hosts(frame, area),
            Tab::Sessions if self.session_detail_id.is_some() => {
                self.render_session_commands(frame, area);
            }
            Tab::Sessions => self.render_sessions(frame, area),
            Tab::Tokens => self.render_tokens(frame, area),
            Tab::Help => self.render_help_tab(frame, area),
        }
        // Empty state overlay
        if self.tab != Tab::Help && self.filtered_row_count() == 0 {
            let msg = if self.filter.is_empty() {
                "no entries"
            } else {
                "no results"
            };
            let p = Paragraph::new(msg)
                .style(Style::default().fg(self.theme.status_default))
                .alignment(ratatui::layout::Alignment::Center);
            let y = area.y + area.height / 2;
            if y < area.y + area.height {
                let msg_area = Rect::new(area.x, y, area.width, 1);
                frame.render_widget(p, msg_area);
            }
        }
    }

    fn row_highlight_style(&self) -> Style {
        let flash = self
            .copied_at
            .is_some_and(|t| t.elapsed() < std::time::Duration::from_millis(500));
        if flash {
            Style::default()
                .bg(self.theme.flash_bg)
                .fg(self.theme.flash_fg)
        } else {
            Style::default().bg(self.theme.row_highlight)
        }
    }

    fn matches_filter(&self, text: &str) -> bool {
        if self.filter.is_empty() {
            return true;
        }
        match_indices(self.match_mode, &self.filter, text).is_some()
    }

    fn fmt_time(&self, dt: chrono::DateTime<chrono::Utc>) -> String {
        if self.relative_time {
            let now = chrono::Utc::now();
            let dur = now.signed_duration_since(dt);
            if dur.num_seconds() < 60 {
                "just now".into()
            } else if dur.num_minutes() < 60 {
                format!("{}m ago", dur.num_minutes())
            } else if dur.num_hours() < 24 {
                format!("{}h ago", dur.num_hours())
            } else if dur.num_days() < 30 {
                format!("{}d ago", dur.num_days())
            } else if dur.num_days() < 365 {
                format!("{}mo ago", dur.num_days() / 30)
            } else {
                format!("{}y ago", dur.num_days() / 365)
            }
        } else {
            dt.format("%Y-%m-%d %H:%M").to_string()
        }
    }

    fn fmt_date(&self, dt: chrono::DateTime<chrono::Utc>) -> String {
        if self.relative_time {
            self.fmt_time(dt)
        } else {
            dt.format("%Y-%m-%d").to_string()
        }
    }

    fn render_frequent(&mut self, frame: &mut Frame, area: Rect) {
        let header = Row::new(vec!["count", "command"]).style(
            Style::default()
                .fg(self.theme.header)
                .add_modifier(Modifier::BOLD),
        );

        let rows: Vec<Row> = self
            .frequent
            .iter()
            .filter(|f| self.matches_filter(&f.command))
            .map(|f| {
                Row::new(vec![
                    Cell::from(f.count.to_string()),
                    Cell::from(f.command.as_str()),
                ])
            })
            .collect();

        let table = Table::new(rows, [Constraint::Length(6), Constraint::Min(20)])
            .header(header)
            .row_highlight_style(self.row_highlight_style());

        frame.render_stateful_widget(table, area, &mut self.table_state);
    }

    fn render_commands(&mut self, frame: &mut Frame, area: Rect) {
        let filter_ref = self.filter.clone();
        let dir_width = dir_col_width(area.width);
        let header = Row::new(vec!["", "timestamp", "command", "directory"]).style(
            Style::default()
                .fg(self.theme.header)
                .add_modifier(Modifier::BOLD),
        );

        let rows: Vec<Row> = self
            .commands
            .iter()
            .map(|c| {
                let cmd_cell = if !filter_ref.is_empty() {
                    Cell::from(highlight_matches(
                        &c.command,
                        &filter_ref,
                        self.match_mode,
                        self.theme.match_highlight,
                    ))
                } else {
                    Cell::from(c.command.as_str())
                };
                Row::new(vec![
                    exit_code_cell(c.exit_code, &self.theme),
                    Cell::from(self.fmt_time(c.timestamp)),
                    cmd_cell,
                    Cell::from(truncate_left(
                        &shorten_dir(&c.directory, &self.home),
                        dir_width as usize,
                    )),
                ])
            })
            .collect();

        let table = Table::new(
            rows,
            [
                Constraint::Length(2),
                Constraint::Length(16),
                Constraint::Min(20),
                Constraint::Length(dir_width),
            ],
        )
        .header(header)
        .row_highlight_style(self.row_highlight_style());

        frame.render_stateful_widget(table, area, &mut self.table_state);
    }

    fn render_local(&mut self, frame: &mut Frame, area: Rect) {
        let filter_ref = self.filter.clone();
        let header = Row::new(vec!["", "timestamp", "command"]).style(
            Style::default()
                .fg(self.theme.header)
                .add_modifier(Modifier::BOLD),
        );

        let rows: Vec<Row> = self
            .local_commands
            .iter()
            .filter(|c| self.matches_filter(&c.command))
            .map(|c| {
                let cmd_cell = if !filter_ref.is_empty() {
                    Cell::from(highlight_matches(
                        &c.command,
                        &filter_ref,
                        self.match_mode,
                        self.theme.match_highlight,
                    ))
                } else {
                    Cell::from(c.command.as_str())
                };
                Row::new(vec![
                    exit_code_cell(c.exit_code, &self.theme),
                    Cell::from(self.fmt_time(c.timestamp)),
                    cmd_cell,
                ])
            })
            .collect();

        let table = Table::new(
            rows,
            [
                Constraint::Length(2),
                Constraint::Length(16),
                Constraint::Min(20),
            ],
        )
        .header(header)
        .row_highlight_style(self.row_highlight_style());

        frame.render_stateful_widget(table, area, &mut self.table_state);
    }

    fn render_aliases(&mut self, frame: &mut Frame, area: Rect) {
        let header = Row::new(vec!["alias", "command", "description", "updated"]).style(
            Style::default()
                .fg(self.theme.header)
                .add_modifier(Modifier::BOLD),
        );

        let rows: Vec<Row> = self
            .aliases
            .iter()
            .filter(|a| self.matches_filter(&a.alias) || self.matches_filter(&a.command))
            .map(|a| {
                Row::new(vec![
                    Cell::from(a.alias.as_str()),
                    Cell::from(truncate(&a.command, 40)),
                    Cell::from(truncate(&a.description, 30)),
                    Cell::from(self.fmt_date(a.date_updated)),
                ])
            })
            .collect();

        let table = Table::new(
            rows,
            [
                Constraint::Length(15),
                Constraint::Min(20),
                Constraint::Length(30),
                Constraint::Length(10),
            ],
        )
        .header(header)
        .row_highlight_style(self.row_highlight_style());

        frame.render_stateful_widget(table, area, &mut self.table_state);
    }

    fn render_hosts(&mut self, frame: &mut Frame, area: Rect) {
        let header = Row::new(vec!["id", "hostname", "created"]).style(
            Style::default()
                .fg(self.theme.header)
                .add_modifier(Modifier::BOLD),
        );

        let rows: Vec<Row> = self
            .hosts
            .iter()
            .filter(|h| self.matches_filter(&h.hostname))
            .map(|h| {
                Row::new(vec![
                    Cell::from(h.id.to_string()),
                    Cell::from(h.hostname.as_str()),
                    Cell::from(self.fmt_time(h.created_at)),
                ])
            })
            .collect();

        let table = Table::new(
            rows,
            [
                Constraint::Length(6),
                Constraint::Min(20),
                Constraint::Length(16),
            ],
        )
        .header(header)
        .row_highlight_style(self.row_highlight_style());

        frame.render_stateful_widget(table, area, &mut self.table_state);
    }

    fn render_sessions(&mut self, frame: &mut Frame, area: Rect) {
        let header = Row::new(vec!["session", "host", "started", "status"]).style(
            Style::default()
                .fg(self.theme.header)
                .add_modifier(Modifier::BOLD),
        );

        let rows: Vec<Row> = self
            .sessions
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let status = s
                    .ended_at
                    .map(|e| self.fmt_time(e))
                    .unwrap_or_else(|| "active".into());
                let cmd_count = self.session_cmd_counts.get(i).copied().unwrap_or(0);
                let id_display = format!("{} ({} cmds)", s.id, cmd_count);
                Row::new(vec![
                    Cell::from(id_display),
                    Cell::from(s.hostname.as_str()),
                    Cell::from(self.fmt_time(s.started_at)),
                    Cell::from(status),
                ])
            })
            .collect();

        let table = Table::new(
            rows,
            [
                Constraint::Min(30),
                Constraint::Length(16),
                Constraint::Length(16),
                Constraint::Length(16),
            ],
        )
        .header(header)
        .row_highlight_style(self.row_highlight_style());

        frame.render_stateful_widget(table, area, &mut self.table_state);
    }

    fn render_session_commands(&mut self, frame: &mut Frame, area: Rect) {
        let dir_width = dir_col_width(area.width);
        let header = Row::new(vec!["", "id", "timestamp", "command", "directory", "r"]).style(
            Style::default()
                .fg(self.theme.header)
                .add_modifier(Modifier::BOLD),
        );

        let rows: Vec<Row> = self
            .session_commands
            .iter()
            .filter(|c| self.matches_filter(&c.command))
            .map(|c| {
                let r = if c.redacted { "Y" } else { "" };
                Row::new(vec![
                    exit_code_cell(c.exit_code, &self.theme),
                    Cell::from(c.id.to_string()),
                    Cell::from(self.fmt_time(c.timestamp)),
                    Cell::from(c.command.as_str()),
                    Cell::from(truncate_left(
                        &shorten_dir(&c.directory, &self.home),
                        dir_width as usize,
                    )),
                    Cell::from(r),
                ])
            })
            .collect();

        let table = Table::new(
            rows,
            [
                Constraint::Length(2),
                Constraint::Length(6),
                Constraint::Length(16),
                Constraint::Min(20),
                Constraint::Length(dir_width),
                Constraint::Length(1),
            ],
        )
        .header(header)
        .row_highlight_style(self.row_highlight_style());

        frame.render_stateful_widget(table, area, &mut self.table_state);
    }

    fn render_tokens(&mut self, frame: &mut Frame, area: Rect) {
        let header = Row::new(vec!["id", "cmd", "type", "placeholder", "value", "created"]).style(
            Style::default()
                .fg(self.theme.header)
                .add_modifier(Modifier::BOLD),
        );

        let rows: Vec<Row> = self
            .tokens
            .iter()
            .filter(|t| self.matches_filter(&t.token_type) || self.matches_filter(&t.placeholder))
            .map(|t| {
                let val = if self.show_values {
                    truncate(&t.original_value, 30)
                } else {
                    "***".into()
                };
                Row::new(vec![
                    Cell::from(t.id.to_string()),
                    Cell::from(t.command_id.to_string()),
                    Cell::from(t.token_type.as_str()),
                    Cell::from(truncate(&t.placeholder, 20)),
                    Cell::from(val),
                    Cell::from(self.fmt_date(t.created_at)),
                ])
            })
            .collect();

        let table = Table::new(
            rows,
            [
                Constraint::Length(5),
                Constraint::Length(5),
                Constraint::Length(12),
                Constraint::Length(20),
                Constraint::Min(10),
                Constraint::Length(10),
            ],
        )
        .header(header)
        .row_highlight_style(self.row_highlight_style());

        frame.render_stateful_widget(table, area, &mut self.table_state);
    }

    fn render_status(&self, frame: &mut Frame, area: Rect) {
        let (left, right, active) = match self.mode {
            Mode::Filter => {
                let left = if self.filter.is_empty() {
                    String::new()
                } else {
                    format!(" / {}_", self.filter)
                };

                let mut right_parts = vec![
                    match self.match_mode {
                        MatchMode::Fuzzy => "fuzzy",
                        MatchMode::Substring => "exact",
                    }
                    .to_string(),
                ];
                let count_info = if self.is_paginated_tab() {
                    let filtered = self.row_count;
                    let total = self.total_paged_rows;
                    if !self.filter.is_empty() && filtered != total {
                        format!("{}:{}", format_thousands(filtered), format_thousands(total))
                    } else {
                        format_thousands(total)
                    }
                } else {
                    let visible = self.filtered_row_count();
                    let total = self.row_count;
                    if !self.filter.is_empty() && visible != total {
                        format!("{}:{}", format_thousands(visible), format_thousands(total))
                    } else {
                        format_thousands(total)
                    }
                };
                right_parts.push(count_info);
                if self.is_paginated_tab() && self.total_pages() > 1 {
                    right_parts.push(format!("pg {}/{}", self.page + 1, self.total_pages()));
                }
                let right = format!("{} ", right_parts.join("  "));
                (left, right, !self.filter.is_empty())
            }
            Mode::Confirm => (String::new(), String::new(), false),
            Mode::EditAlias => {
                let field = match self.edit_field {
                    EditField::Command => "command",
                    EditField::Description => "description",
                };
                (
                    format!(
                        " editing {} [{}] | Tab=switch Enter=save Esc=cancel",
                        self.edit_alias_name, field
                    ),
                    String::new(),
                    true,
                )
            }
        };

        let width = area.width as usize;
        let left_len = left.chars().count();
        let right_len = right.chars().count();
        let pad = width.saturating_sub(left_len + right_len);

        let style_left = if active {
            Style::default().fg(self.theme.status_active)
        } else {
            Style::default().fg(self.theme.status_default)
        };
        let style_right = Style::default().fg(self.theme.status_default);

        let line = Line::from(vec![
            Span::styled(left, style_left),
            Span::raw(" ".repeat(pad)),
            Span::styled(right, style_right),
        ]);
        frame.render_widget(Paragraph::new(line), area);
    }

    fn render_confirm(&self, frame: &mut Frame, area: Rect) {
        let block_area = centered_rect(50, 5, area);
        let text = format!("{} (y/n)", self.confirm_msg);
        let popup = Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("confirm delete")
                    .style(Style::default().fg(self.theme.popup_confirm)),
            )
            .style(Style::default().fg(self.theme.popup_text))
            .wrap(Wrap { trim: false });
        // Clear the area behind the popup
        frame.render_widget(ratatui::widgets::Clear, block_area);
        frame.render_widget(popup, block_area);
    }

    fn render_edit_alias(&self, frame: &mut Frame, area: Rect) {
        let block_area = centered_rect(60, 7, area);
        let field_name = match self.edit_field {
            EditField::Command => "Command",
            EditField::Description => "Description",
        };
        let title = format!("Edit Alias '{}' — {}", self.edit_alias_name, field_name);
        let text = format!(
            "{}_\n\nTab=switch field  Enter=save  Esc=cancel",
            self.edit_buf
        );
        let popup = Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .style(Style::default().fg(self.theme.popup_accent)),
            )
            .style(Style::default().fg(self.theme.popup_text))
            .wrap(Wrap { trim: false });
        frame.render_widget(ratatui::widgets::Clear, block_area);
        frame.render_widget(popup, block_area);
    }

    fn render_help_tab(&self, frame: &mut Frame, area: Rect) {
        let header_style = Style::default()
            .fg(self.theme.header)
            .add_modifier(Modifier::BOLD);

        let mut help = vec![
            Line::from(vec![Span::styled("search", header_style)]),
            Line::from("  type to filter          ^F  toggle fuzzy / exact matching"),
            Line::from("  Enter  run command      Shift+Enter / Alt+Enter  edit before running"),
            Line::from("  Esc    clear / quit"),
            Line::from(""),
            Line::from(vec![Span::styled("navigation", header_style)]),
            Line::from("  ↑/↓       move up/down"),
            Line::from("  ←/→       page prev/next (History, Sessions)"),
            Line::from("  Tab       next tab           Shift+Tab  prev tab"),
            Line::from("  Alt+1..8  jump to tab"),
            Line::from(""),
            Line::from(vec![Span::styled("quick jump", header_style)]),
            Line::from("  ^H  History        ^L  Local (cwd)     ^S  Current session"),
            Line::from(""),
            Line::from(vec![Span::styled("actions", header_style)]),
            Line::from("  ^T  toggle relative time"),
            Line::from("  ^D  delete selected"),
            Line::from("  ^E  edit alias (Aliases tab)"),
            Line::from("  ^V  reveal token values (Tokens tab)"),
            Line::from("  ^Y  copy selected command to clipboard"),
            Line::from("  ^C  quit"),
        ];

        if let Ok(stats) = self.db.get_stats() {
            help.push(Line::from(""));
            help.push(Line::from(vec![Span::styled(
                "database stats",
                header_style,
            )]));
            help.push(Line::from(format!(
                "  Commands: {}    Sessions: {}    Hosts: {}",
                format_thousands(stats.total_commands),
                format_thousands(stats.total_sessions),
                format_thousands(stats.total_hosts),
            )));
            help.push(Line::from(format!(
                "  Redacted: {}    Tokens: {}",
                format_thousands(stats.redacted_commands),
                format_thousands(stats.stored_tokens),
            )));
            if let (Some(oldest), Some(newest)) = (stats.oldest_entry, stats.newest_entry) {
                help.push(Line::from(format!(
                    "  Date range: {}  to  {}",
                    oldest.format("%Y-%m-%d %H:%M"),
                    newest.format("%Y-%m-%d %H:%M"),
                )));
            }
        }

        let p = Paragraph::new(help).style(Style::default().fg(self.theme.tab_text));
        frame.render_widget(p, area);
    }
}

fn dir_col_width(term_width: u16) -> u16 {
    // ~25% of terminal width, clamped to 20..60
    ((term_width as u32) / 4).clamp(20, 60) as u16
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() > max {
        format!("{}...", &s[..max.saturating_sub(3)])
    } else {
        s.to_string()
    }
}

fn truncate_left(s: &str, max: usize) -> String {
    if s.len() > max {
        format!("...{}", &s[s.len() - max.saturating_sub(3)..])
    } else {
        s.to_string()
    }
}

fn shorten_dir(path: &str, home: &str) -> String {
    if !home.is_empty()
        && let Some(rest) = path.strip_prefix(home)
    {
        return format!("~{rest}");
    }
    path.to_string()
}

fn exit_code_cell(exit_code: Option<i32>, theme: &Theme) -> Cell<'static> {
    match exit_code {
        None | Some(0) => Cell::from(Span::styled("\u{2713}", Style::default().fg(theme.success))),
        Some(_) => Cell::from(Span::styled("\u{2717}", Style::default().fg(theme.error))),
    }
}

/// Lowercase char-by-char so indices stay aligned with `text.chars()`.
fn lower_chars(s: &str) -> Vec<char> {
    s.chars()
        .map(|c| c.to_lowercase().next().unwrap_or(c))
        .collect()
}

/// Case-insensitive match of `pattern` in `text`, returning the char indices
/// of matched characters.
///
/// - `Fuzzy`: each pattern char must appear in order, not necessarily contiguously.
/// - `Substring`: the pattern must appear contiguously.
fn match_indices(mode: MatchMode, pattern: &str, text: &str) -> Option<Vec<usize>> {
    let pat = lower_chars(pattern);
    let txt = lower_chars(text);
    match mode {
        MatchMode::Fuzzy => {
            let mut indices = Vec::with_capacity(pat.len());
            let mut it = txt.iter().enumerate();
            for p in &pat {
                let (i, _) = it.by_ref().find(|(_, c)| *c == p)?;
                indices.push(i);
            }
            Some(indices)
        }
        MatchMode::Substring => {
            if pat.is_empty() {
                return Some(Vec::new());
            }
            txt.windows(pat.len())
                .position(|w| w == pat.as_slice())
                .map(|start| (start..start + pat.len()).collect())
        }
    }
}

fn highlight_matches<'a>(
    text: &'a str,
    filter: &str,
    mode: MatchMode,
    highlight_color: Color,
) -> Line<'a> {
    if filter.is_empty() {
        return Line::from(text);
    }
    let Some(indices) = match_indices(mode, filter, text) else {
        return Line::from(text);
    };
    let highlight_set: std::collections::HashSet<usize> = indices.into_iter().collect();
    let text_chars: Vec<char> = text.chars().collect();
    let mut spans: Vec<Span<'a>> = Vec::new();
    let mut buf = String::new();
    let mut in_highlight = false;

    for (i, &ch) in text_chars.iter().enumerate() {
        let is_match = highlight_set.contains(&i);
        if is_match != in_highlight {
            if !buf.is_empty() {
                if in_highlight {
                    spans.push(Span::styled(
                        std::mem::take(&mut buf),
                        Style::default()
                            .add_modifier(Modifier::BOLD)
                            .fg(highlight_color),
                    ));
                } else {
                    spans.push(Span::raw(std::mem::take(&mut buf)));
                }
            }
            in_highlight = is_match;
        }
        buf.push(ch);
    }
    if !buf.is_empty() {
        if in_highlight {
            spans.push(Span::styled(
                buf,
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(highlight_color),
            ));
        } else {
            spans.push(Span::raw(buf));
        }
    }
    Line::from(spans)
}

fn format_thousands(n: usize) -> String {
    let s = n.to_string();
    let mut result = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            result.push(',');
        }
        result.push(c);
    }
    result
}

fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length((area.height.saturating_sub(height)) / 2),
            Constraint::Length(height),
            Constraint::Min(0),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

/// A command picked in the TUI and what the shell should do with it.
pub enum Selection {
    /// Run the command immediately (Enter).
    Execute(String),
    /// Place the command on the prompt for editing (Shift+Enter / Alt+Enter).
    Edit(String),
}

/// Run the interactive TUI for browsing database entities.
/// Returns the selected command, if any, and whether it should be executed.
pub fn run_tui(db: &Database, cwd: String) -> Result<Option<Selection>> {
    let mut tty = File::options().write(true).open("/dev/tty")?;
    enable_raw_mode()?;
    // Ask for disambiguated key reporting so Shift+Enter is distinguishable
    // from Enter. Terminals without support ignore the sequence.
    execute!(
        tty,
        EnterAlternateScreen,
        PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
    )?;
    let backend = CrosstermBackend::new(tty);
    let mut terminal = Terminal::new(backend)?;

    let mut app = AppTUI::new(db, cwd)?;

    let result = (|| -> Result<()> {
        while app.running {
            terminal.draw(|f| app.render(f))?;

            if event::poll(std::time::Duration::from_millis(100))?
                && let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                app.handle_key(key)?;
            }
        }
        Ok(())
    })();

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        PopKeyboardEnhancementFlags,
        LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;

    result?;
    Ok(app.selected_command.map(|cmd| {
        if app.execute_selected {
            Selection::Execute(cmd)
        } else {
            Selection::Edit(cmd)
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_matches_subsequence() {
        assert_eq!(
            match_indices(MatchMode::Fuzzy, "gco", "git checkout"),
            Some(vec![0, 4, 9])
        );
        assert_eq!(
            match_indices(MatchMode::Fuzzy, "GCO", "git checkout").map(|v| v.len()),
            Some(3)
        );
        assert_eq!(match_indices(MatchMode::Fuzzy, "xyz", "git checkout"), None);
    }

    #[test]
    fn substring_requires_contiguous_match() {
        assert_eq!(
            match_indices(MatchMode::Substring, "check", "git checkout"),
            Some(vec![4, 5, 6, 7, 8])
        );
        assert_eq!(
            match_indices(MatchMode::Substring, "gco", "git checkout"),
            None
        );
    }
}
