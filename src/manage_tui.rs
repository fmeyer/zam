//! Management TUI for interactive history browsing and editing
//!
//! Provides a full-screen interface for managing command history:
//! - Browse entries with details
//! - Delete entries
//! - View command details
//! - Filter/search

use crate::error::Result;
use crate::history::HistoryEntry;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};
use std::io;

/// Actions that can be performed on entries
#[derive(Debug, Clone)]
pub enum ManageAction {
    /// Delete the entry at the given index
    Delete(usize),
    /// No action
    None,
}

/// Management UI state
pub struct ManagementUI {
    /// All entries being managed
    entries: Vec<HistoryEntry>,
    /// Indices to delete
    to_delete: Vec<usize>,
    /// Current selection
    selected: usize,
    /// List state for rendering
    list_state: ListState,
    /// Search filter
    filter: String,
    /// Filtered indices
    filtered_indices: Vec<usize>,
    /// Whether UI is running
    running: bool,
    /// Show help panel
    show_help: bool,
}

impl ManagementUI {
    pub fn new(entries: Vec<HistoryEntry>) -> Self {
        let filtered_indices: Vec<usize> = (0..entries.len()).collect();
        let mut ui = Self {
            entries,
            to_delete: Vec::new(),
            selected: 0,
            list_state: ListState::default(),
            filter: String::new(),
            filtered_indices,
            running: true,
            show_help: false,
        };
        ui.list_state.select(Some(0));
        ui
    }

    /// Update filter and rebuild filtered indices
    fn update_filter(&mut self, filter: String) {
        self.filter = filter;
        if self.filter.is_empty() {
            self.filtered_indices = (0..self.entries.len()).collect();
        } else {
            let filter_lower = self.filter.to_lowercase();
            self.filtered_indices = self
                .entries
                .iter()
                .enumerate()
                .filter(|(_, e)| e.command.to_lowercase().contains(&filter_lower))
                .map(|(i, _)| i)
                .collect();
        }
        self.selected = 0;
        self.list_state.select(Some(0));
    }

    fn select_previous(&mut self) {
        if !self.filtered_indices.is_empty() {
            self.selected = self.selected.saturating_sub(1);
            self.list_state.select(Some(self.selected));
        }
    }

    fn select_next(&mut self) {
        if !self.filtered_indices.is_empty() {
            self.selected = (self.selected + 1).min(self.filtered_indices.len() - 1);
            self.list_state.select(Some(self.selected));
        }
    }

    fn toggle_delete_current(&mut self) {
        if let Some(&idx) = self.filtered_indices.get(self.selected) {
            if let Some(pos) = self.to_delete.iter().position(|&i| i == idx) {
                self.to_delete.remove(pos);
            } else {
                self.to_delete.push(idx);
            }
        }
    }

    fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.running = false;
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.running = false;
            }
            KeyCode::Char('?') | KeyCode::F(1) => {
                self.show_help = !self.show_help;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.select_previous();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.select_next();
            }
            KeyCode::Char('d') | KeyCode::Delete => {
                self.toggle_delete_current();
            }
            KeyCode::Char('/') => {
                // Start search mode - for now just clear filter
                self.update_filter(String::new());
            }
            KeyCode::Char(c) if !self.filter.is_empty() || c == '/' => {
                if c != '/' {
                    self.filter.push(c);
                    self.update_filter(self.filter.clone());
                }
            }
            KeyCode::Backspace if !self.filter.is_empty() => {
                self.filter.pop();
                self.update_filter(self.filter.clone());
            }
            _ => {}
        }
    }

    fn render(&mut self, frame: &mut Frame) {
        let chunks = if self.show_help {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),   // Header
                    Constraint::Percentage(50), // List
                    Constraint::Percentage(50), // Help
                ])
                .split(frame.area())
        } else {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),   // Header
                    Constraint::Min(10),     // List
                    Constraint::Length(5),   // Details
                ])
                .split(frame.area())
        };

        // Header
        let title = if !self.filter.is_empty() {
            format!("History Manager - Filter: {} ({} matches)", self.filter, self.filtered_indices.len())
        } else {
            format!("History Manager ({} entries, {} marked for deletion)", self.entries.len(), self.to_delete.len())
        };
        let header = Paragraph::new(title)
            .block(Block::default().borders(Borders::ALL))
            .style(Style::default().fg(Color::Cyan));
        frame.render_widget(header, chunks[0]);

        // Entry list
        let items: Vec<ListItem> = self
            .filtered_indices
            .iter()
            .map(|&idx| {
                let entry = &self.entries[idx];
                let timestamp = entry.timestamp.format("%Y-%m-%d %H:%M");
                let marked = if self.to_delete.contains(&idx) { "[MARK] " } else { "" };
                let deleted = if entry.deleted { "[DELETED] " } else { "" };
                let redacted = if entry.redacted { "[R] " } else { "" };

                let line = Line::from(vec![
                    Span::styled(format!("{}{}{}", deleted, marked, redacted), Style::default().fg(Color::Red)),
                    Span::styled(format!("{} ", timestamp), Style::default().fg(Color::DarkGray)),
                    Span::styled(&entry.command, if entry.deleted {
                        Style::default().fg(Color::DarkGray)
                    } else {
                        Style::default().fg(Color::White)
                    }),
                ]);
                ListItem::new(line)
            })
            .collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Commands"))
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> ");

        frame.render_stateful_widget(list, chunks[1], &mut self.list_state);

        // Details or Help
        if self.show_help {
            let help_text = vec![
                "Keybindings:",
                "",
                "  ↑/k       - Move up",
                "  ↓/j       - Move down",
                "  d/Delete  - Mark/unmark for deletion",
                "  /         - Start filter",
                "  Backspace - Clear filter",
                "  Enter     - Confirm deletions and exit",
                "  ?/F1      - Toggle help",
                "  q/Esc     - Quit without deleting",
                "  Ctrl+C    - Quit without deleting",
            ];
            let help = Paragraph::new(help_text.join("\n"))
                .block(Block::default().borders(Borders::ALL).title("Help"))
                .style(Style::default().fg(Color::Yellow))
                .wrap(Wrap { trim: false });
            frame.render_widget(help, chunks[2]);
        } else if let Some(&idx) = self.filtered_indices.get(self.selected) {
            if let Some(entry) = self.entries.get(idx) {
                let details = format!(
                    "Command: {}\nDirectory: {}\nTimestamp: {}\nRedacted: {}\nMarked for deletion: {}",
                    entry.command,
                    entry.directory,
                    entry.timestamp.format("%Y-%m-%d %H:%M:%S"),
                    if entry.redacted { "Yes" } else { "No" },
                    if self.to_delete.contains(&idx) { "Yes" } else { "No" }
                );
                let details_widget = Paragraph::new(details)
                    .block(Block::default().borders(Borders::ALL).title("Details"))
                    .style(Style::default().fg(Color::Green))
                    .wrap(Wrap { trim: false });
                frame.render_widget(details_widget, chunks[2]);
            }
        }
    }

    pub fn get_deletions(&self) -> Vec<usize> {
        self.to_delete.clone()
    }
}

/// Run the management TUI and return indices to delete
pub fn run_management_ui(entries: Vec<HistoryEntry>) -> Result<Vec<usize>> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut ui = ManagementUI::new(entries);

    // Main event loop
    let result = (|| -> Result<()> {
        while ui.running {
            terminal.draw(|f| ui.render(f))?;

            if event::poll(std::time::Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    // Check if Enter was pressed
                    if matches!(key.code, KeyCode::Enter) {
                        ui.running = false;
                        break;
                    }
                    ui.handle_key(key);
                }
            }
        }
        Ok(())
    })();

    // Always restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result?;
    Ok(ui.get_deletions())
}
