//! Interactive TUI for browsing and managing all database entities

use crate::database::{Alias, CommandEntry, Database, Host, Session, Token};
use crate::error::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState, Tabs, Wrap},
};
use std::fs::File;

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Local,
    Frequent,
    Commands,
    Aliases,
    Hosts,
    Sessions,
    Tokens,
}

const TABS: [Tab; 7] = [
    Tab::Local,
    Tab::Frequent,
    Tab::Commands,
    Tab::Aliases,
    Tab::Hosts,
    Tab::Sessions,
    Tab::Tokens,
];

impl Tab {
    fn title(self) -> &'static str {
        match self {
            Tab::Local => "Local",
            Tab::Frequent => "Top 50",
            Tab::Commands => "History",
            Tab::Aliases => "Aliases",
            Tab::Hosts => "Hosts",
            Tab::Sessions => "Sessions",
            Tab::Tokens => "Tokens",
        }
    }

    fn index(self) -> usize {
        TABS.iter().position(|&t| t == self).unwrap_or(0)
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Normal,
    Filter,
    Confirm,
    EditAlias,
    Help,
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
    tab: Tab,
    mode: Mode,

    // Data
    commands: Vec<CommandEntry>,
    local_commands: Vec<CommandEntry>,
    frequent: Vec<FrequentCommand>,
    aliases: Vec<Alias>,
    hosts: Vec<Host>,
    sessions: Vec<Session>,
    tokens: Vec<Token>,

    // Pagination (Commands tab)
    page: usize,
    page_size: usize,
    total_commands: usize,

    // Table state per tab
    table_state: TableState,
    row_count: usize,

    // Filter
    filter: String,

    // Confirm delete
    confirm_msg: String,

    // Edit alias
    edit_field: EditField,
    edit_buf: String,
    edit_alias_name: String,

    // Status
    status: Option<String>,
    show_values: bool,
    running: bool,
    selected_command: Option<String>,
}

impl<'a> AppTUI<'a> {
    fn new(db: &'a Database, cwd: String) -> Result<Self> {
        let mut app = Self {
            db,
            cwd,
            tab: Tab::Local,
            mode: Mode::Normal,
            commands: Vec::new(),
            local_commands: Vec::new(),
            frequent: Vec::new(),
            aliases: Vec::new(),
            hosts: Vec::new(),
            sessions: Vec::new(),
            tokens: Vec::new(),
            page: 0,
            page_size: 100,
            total_commands: 0,
            table_state: TableState::default(),
            row_count: 0,
            filter: String::new(),
            confirm_msg: String::new(),
            edit_field: EditField::Command,
            edit_buf: String::new(),
            edit_alias_name: String::new(),
            status: None,
            show_values: false,
            running: true,
            selected_command: None,
        };
        app.load_tab()?;
        Ok(app)
    }

    fn load_tab(&mut self) -> Result<()> {
        self.table_state = TableState::default();
        match self.tab {
            Tab::Commands => {
                self.total_commands = self.db.count_unique_commands()?;
                self.commands = self
                    .db
                    .get_unique_commands_paginated(self.page * self.page_size, self.page_size)?;
                self.row_count = self.commands.len();
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
            Tab::Sessions => {
                self.sessions = self.db.get_all_sessions()?;
                self.row_count = self.sessions.len();
            }
            Tab::Tokens => {
                self.tokens = self.db.get_all_tokens()?;
                self.row_count = self.tokens.len();
            }
        }
        if self.row_count > 0 {
            self.table_state.select(Some(0));
        }
        Ok(())
    }

    fn selected_index(&self) -> Option<usize> {
        self.table_state.selected()
    }

    fn select_prev(&mut self) {
        if self.row_count == 0 {
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
        if self.row_count == 0 {
            return;
        }
        let i = self
            .table_state
            .selected()
            .map(|s| (s + 1).min(self.row_count - 1))
            .unwrap_or(0);
        self.table_state.select(Some(i));
    }

    fn next_tab(&mut self) -> Result<()> {
        let idx = (self.tab.index() + 1) % TABS.len();
        self.tab = TABS[idx];
        self.filter.clear();
        self.page = 0;
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
        self.load_tab()
    }

    fn request_delete(&mut self) {
        let Some(idx) = self.selected_index() else {
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
            Tab::Frequent => return,
        };
        self.confirm_msg = msg;
        self.mode = Mode::Confirm;
    }

    fn confirm_delete(&mut self) -> Result<()> {
        let Some(idx) = self.selected_index() else {
            self.mode = Mode::Normal;
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
            Tab::Frequent => {}
        }
        self.mode = Mode::Normal;
        self.load_tab()
    }

    fn total_pages(&self) -> usize {
        if self.total_commands == 0 {
            1
        } else {
            self.total_commands.div_ceil(self.page_size)
        }
    }

    fn next_page(&mut self) -> Result<()> {
        if self.tab != Tab::Commands {
            return Ok(());
        }
        if self.page + 1 < self.total_pages() {
            self.page += 1;
            self.load_tab()?;
        }
        Ok(())
    }

    fn prev_page(&mut self) -> Result<()> {
        if self.tab != Tab::Commands {
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
        let Some(idx) = self.selected_index() else {
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
            self.mode = Mode::Normal;
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
        self.mode = Mode::Normal;
        self.load_tab()
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        match self.mode {
            Mode::Help => {
                self.mode = Mode::Normal;
            }
            Mode::Confirm => match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => self.confirm_delete()?,
                _ => self.mode = Mode::Normal,
            },
            Mode::EditAlias => match key.code {
                KeyCode::Esc => {
                    self.mode = Mode::Normal;
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
                        self.mode = Mode::Normal;
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
                    self.filter.clear();
                    self.mode = Mode::Normal;
                }
                KeyCode::Enter => {
                    self.mode = Mode::Normal;
                }
                KeyCode::Backspace => {
                    self.filter.pop();
                }
                KeyCode::Char(c) => {
                    self.filter.push(c);
                }
                _ => {}
            },
            Mode::Normal => match key.code {
                KeyCode::Char('q') | KeyCode::Esc => {
                    self.running = false;
                }
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.running = false;
                }
                KeyCode::Tab => self.next_tab()?,
                KeyCode::BackTab => self.prev_tab()?,
                KeyCode::Char('1') => {
                    self.tab = Tab::Local;
                    self.filter.clear();
                    self.page = 0;
                    self.load_tab()?;
                }
                KeyCode::Char('2') => {
                    self.tab = Tab::Frequent;
                    self.filter.clear();
                    self.page = 0;
                    self.load_tab()?;
                }
                KeyCode::Char('3') => {
                    self.tab = Tab::Commands;
                    self.filter.clear();
                    self.page = 0;
                    self.load_tab()?;
                }
                KeyCode::Char('4') => {
                    self.tab = Tab::Aliases;
                    self.filter.clear();
                    self.page = 0;
                    self.load_tab()?;
                }
                KeyCode::Char('5') => {
                    self.tab = Tab::Hosts;
                    self.filter.clear();
                    self.page = 0;
                    self.load_tab()?;
                }
                KeyCode::Char('6') => {
                    self.tab = Tab::Sessions;
                    self.filter.clear();
                    self.page = 0;
                    self.load_tab()?;
                }
                KeyCode::Char('7') => {
                    self.tab = Tab::Tokens;
                    self.filter.clear();
                    self.page = 0;
                    self.load_tab()?;
                }
                KeyCode::Up | KeyCode::Char('k') => self.select_prev(),
                KeyCode::Down | KeyCode::Char('j') => self.select_next(),
                KeyCode::Right | KeyCode::Char(']') => self.next_page()?,
                KeyCode::Left | KeyCode::Char('[') => self.prev_page()?,
                KeyCode::Char('/') => {
                    self.filter.clear();
                    self.mode = Mode::Filter;
                }
                KeyCode::Enter if self.tab == Tab::Frequent => {
                    if let Some(idx) = self.selected_index()
                        && let Some(f) = self.frequent.get(idx)
                    {
                        self.selected_command = Some(f.command.clone());
                        self.running = false;
                    }
                }
                KeyCode::Enter if self.tab == Tab::Local => {
                    if let Some(idx) = self.selected_index()
                        && let Some(cmd) = self.local_commands.get(idx)
                    {
                        self.selected_command = Some(cmd.command.clone());
                        self.running = false;
                    }
                }
                KeyCode::Char('d') | KeyCode::Delete => self.request_delete(),
                KeyCode::Char('e') if self.tab == Tab::Aliases => {
                    self.start_edit_alias();
                }
                KeyCode::Char('v') if self.tab == Tab::Tokens => {
                    self.show_values = !self.show_values;
                }
                KeyCode::Char('?') => {
                    self.mode = Mode::Help;
                }
                _ => {}
            },
        }
        Ok(())
    }

    fn render(&mut self, frame: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Tab bar
                Constraint::Min(5),    // Table
                Constraint::Length(1), // Status
            ])
            .split(frame.area());

        self.render_tabs(frame, chunks[0]);
        self.render_table(frame, chunks[1]);
        self.render_status(frame, chunks[2]);

        match self.mode {
            Mode::Confirm => self.render_confirm(frame, frame.area()),
            Mode::EditAlias => self.render_edit_alias(frame, frame.area()),
            Mode::Help => self.render_help(frame, frame.area()),
            _ => {}
        }
    }

    fn render_tabs(&self, frame: &mut Frame, area: Rect) {
        let titles: Vec<Line> = TABS
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let num = format!("{}:", i + 1);
                Line::from(vec![
                    Span::styled(num, Style::default().fg(Color::DarkGray)),
                    Span::raw(t.title()),
                ])
            })
            .collect();

        let tabs = Tabs::new(titles)
            .block(Block::default().borders(Borders::ALL).title("zam"))
            .select(self.tab.index())
            .style(Style::default().fg(Color::White))
            .highlight_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .divider("|");

        frame.render_widget(tabs, area);
    }

    fn render_table(&mut self, frame: &mut Frame, area: Rect) {
        match self.tab {
            Tab::Local => self.render_local(frame, area),
            Tab::Frequent => self.render_frequent(frame, area),
            Tab::Commands => self.render_commands(frame, area),
            Tab::Aliases => self.render_aliases(frame, area),
            Tab::Hosts => self.render_hosts(frame, area),
            Tab::Sessions => self.render_sessions(frame, area),
            Tab::Tokens => self.render_tokens(frame, area),
        }
    }

    fn matches_filter(&self, text: &str) -> bool {
        if self.filter.is_empty() {
            return true;
        }
        text.to_lowercase().contains(&self.filter.to_lowercase())
    }

    fn render_frequent(&mut self, frame: &mut Frame, area: Rect) {
        let header = Row::new(vec!["#", "Count", "Command"]).style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );

        let rows: Vec<Row> = self
            .frequent
            .iter()
            .enumerate()
            .filter(|(_, f)| self.matches_filter(&f.command))
            .map(|(i, f)| {
                Row::new(vec![
                    Cell::from((i + 1).to_string()),
                    Cell::from(f.count.to_string()),
                    Cell::from(f.command.chars().take(80).collect::<String>()),
                ])
            })
            .collect();

        let table = Table::new(
            rows,
            [
                Constraint::Length(4),
                Constraint::Length(6),
                Constraint::Min(20),
            ],
        )
        .header(header)
        .block(Block::default().borders(Borders::ALL).title("Top 50"))
        .row_highlight_style(Style::default().bg(Color::DarkGray));

        frame.render_stateful_widget(table, area, &mut self.table_state);
    }

    fn render_commands(&mut self, frame: &mut Frame, area: Rect) {
        let header = Row::new(vec!["ID", "Timestamp", "Command", "Directory", "R"]).style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );

        let rows: Vec<Row> = self
            .commands
            .iter()
            .filter(|c| self.matches_filter(&c.command))
            .map(|c| {
                let r = if c.redacted { "Y" } else { "" };
                Row::new(vec![
                    Cell::from(c.id.to_string()),
                    Cell::from(c.timestamp.format("%Y-%m-%d %H:%M").to_string()),
                    Cell::from(c.command.chars().take(60).collect::<String>()),
                    Cell::from(truncate(&c.directory, 25)),
                    Cell::from(r),
                ])
            })
            .collect();

        let table = Table::new(
            rows,
            [
                Constraint::Length(6),
                Constraint::Length(16),
                Constraint::Min(20),
                Constraint::Length(25),
                Constraint::Length(1),
            ],
        );
        let title = format!(
            "History (page {}/{}, {} total)",
            self.page + 1,
            self.total_pages(),
            self.total_commands,
        );

        let table = table
            .header(header)
            .block(Block::default().borders(Borders::ALL).title(title))
            .row_highlight_style(Style::default().bg(Color::DarkGray));

        frame.render_stateful_widget(table, area, &mut self.table_state);
    }

    fn render_local(&mut self, frame: &mut Frame, area: Rect) {
        let header = Row::new(vec!["ID", "Timestamp", "Command", "R"]).style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );

        let rows: Vec<Row> = self
            .local_commands
            .iter()
            .filter(|c| self.matches_filter(&c.command))
            .map(|c| {
                let r = if c.redacted { "Y" } else { "" };
                Row::new(vec![
                    Cell::from(c.id.to_string()),
                    Cell::from(c.timestamp.format("%Y-%m-%d %H:%M").to_string()),
                    Cell::from(c.command.chars().take(80).collect::<String>()),
                    Cell::from(r),
                ])
            })
            .collect();

        let title = format!("Local — {}", self.cwd);
        let table = Table::new(
            rows,
            [
                Constraint::Length(6),
                Constraint::Length(16),
                Constraint::Min(20),
                Constraint::Length(1),
            ],
        )
        .header(header)
        .block(Block::default().borders(Borders::ALL).title(title))
        .row_highlight_style(Style::default().bg(Color::DarkGray));

        frame.render_stateful_widget(table, area, &mut self.table_state);
    }

    fn render_aliases(&mut self, frame: &mut Frame, area: Rect) {
        let header = Row::new(vec!["Alias", "Command", "Description", "Updated"]).style(
            Style::default()
                .fg(Color::Yellow)
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
                    Cell::from(a.date_updated.format("%Y-%m-%d").to_string()),
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
        .block(Block::default().borders(Borders::ALL).title("Aliases"))
        .row_highlight_style(Style::default().bg(Color::DarkGray));

        frame.render_stateful_widget(table, area, &mut self.table_state);
    }

    fn render_hosts(&mut self, frame: &mut Frame, area: Rect) {
        let header = Row::new(vec!["ID", "Hostname", "Created"]).style(
            Style::default()
                .fg(Color::Yellow)
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
                    Cell::from(h.created_at.format("%Y-%m-%d %H:%M").to_string()),
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
        .block(Block::default().borders(Borders::ALL).title("Hosts"))
        .row_highlight_style(Style::default().bg(Color::DarkGray));

        frame.render_stateful_widget(table, area, &mut self.table_state);
    }

    fn render_sessions(&mut self, frame: &mut Frame, area: Rect) {
        let header = Row::new(vec!["ID", "Host", "Started", "Ended"]).style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );

        let rows: Vec<Row> = self
            .sessions
            .iter()
            .filter(|s| self.matches_filter(s.id.as_ref()))
            .map(|s| {
                let ended = s
                    .ended_at
                    .map(|e| e.format("%Y-%m-%d %H:%M").to_string())
                    .unwrap_or_else(|| "active".into());
                Row::new(vec![
                    Cell::from(truncate(s.id.as_ref(), 12)),
                    Cell::from(s.host_id.to_string()),
                    Cell::from(s.started_at.format("%Y-%m-%d %H:%M").to_string()),
                    Cell::from(ended),
                ])
            })
            .collect();

        let table = Table::new(
            rows,
            [
                Constraint::Length(14),
                Constraint::Length(6),
                Constraint::Length(16),
                Constraint::Min(16),
            ],
        )
        .header(header)
        .block(Block::default().borders(Borders::ALL).title("Sessions"))
        .row_highlight_style(Style::default().bg(Color::DarkGray));

        frame.render_stateful_widget(table, area, &mut self.table_state);
    }

    fn render_tokens(&mut self, frame: &mut Frame, area: Rect) {
        let title = if self.show_values {
            "Tokens (v=hide values)"
        } else {
            "Tokens (v=show values)"
        };
        let header = Row::new(vec!["ID", "Cmd", "Type", "Placeholder", "Value", "Created"]).style(
            Style::default()
                .fg(Color::Yellow)
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
                    Cell::from(t.created_at.format("%Y-%m-%d").to_string()),
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
        .block(Block::default().borders(Borders::ALL).title(title))
        .row_highlight_style(Style::default().bg(Color::DarkGray));

        frame.render_stateful_widget(table, area, &mut self.table_state);
    }

    fn render_status(&self, frame: &mut Frame, area: Rect) {
        let text = match self.mode {
            Mode::Filter => format!(" /{}_ | Esc=cancel Enter=done", self.filter),
            Mode::Confirm | Mode::Help => String::new(),
            Mode::EditAlias => {
                let field = match self.edit_field {
                    EditField::Command => "command",
                    EditField::Description => "description",
                };
                format!(
                    " Editing {} [{}] | Tab=switch field Enter=save Esc=cancel",
                    self.edit_alias_name, field
                )
            }
            Mode::Normal => {
                let extra = match self.tab {
                    Tab::Aliases => " e=edit",
                    Tab::Tokens => " v=reveal",
                    Tab::Commands => " [/]=page",
                    _ => "",
                };
                if let Some(ref msg) = self.status {
                    format!(" {} | q=quit Tab=switch /=filter d=delete{}", msg, extra)
                } else {
                    format!(" q=quit Tab=switch /=filter d=delete{} ?=help", extra)
                }
            }
        };

        let style = match self.mode {
            Mode::Filter | Mode::EditAlias => Style::default().fg(Color::Yellow),
            _ => Style::default().fg(Color::DarkGray),
        };

        let status = Paragraph::new(text).style(style);
        frame.render_widget(status, area);
    }

    fn render_confirm(&self, frame: &mut Frame, area: Rect) {
        let block_area = centered_rect(50, 5, area);
        let text = format!("{} (y/n)", self.confirm_msg);
        let popup = Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Confirm Delete")
                    .style(Style::default().fg(Color::Red)),
            )
            .style(Style::default().fg(Color::White))
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
                    .style(Style::default().fg(Color::Cyan)),
            )
            .style(Style::default().fg(Color::White))
            .wrap(Wrap { trim: false });
        frame.render_widget(ratatui::widgets::Clear, block_area);
        frame.render_widget(popup, block_area);
    }

    fn render_help(&self, frame: &mut Frame, area: Rect) {
        let block_area = centered_rect(60, 16, area);
        let help_text = "\
Navigation
  j/↓  Move down          k/↑  Move up
  Tab  Next tab         S-Tab  Previous tab
  1-7  Jump to tab
  [/←  Previous page     ]/→  Next page (History)

Actions
  /    Filter rows         d    Delete selected
  e    Edit alias (Aliases tab only)
  v    Toggle token values (Tokens tab only)

General
  q    Quit              Esc    Cancel / Quit
  ?    This help

Press any key to close";
        let popup = Paragraph::new(help_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Help")
                    .style(Style::default().fg(Color::Cyan)),
            )
            .style(Style::default().fg(Color::White));
        frame.render_widget(ratatui::widgets::Clear, block_area);
        frame.render_widget(popup, block_area);
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() > max {
        format!("{}...", &s[..max.saturating_sub(3)])
    } else {
        s.to_string()
    }
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

/// Run the interactive TUI for browsing database entities.
/// Returns the selected command string if the user pressed Enter on a Local entry.
pub fn run_tui(db: &Database, cwd: String) -> Result<Option<String>> {
    let mut tty = File::options().write(true).open("/dev/tty")?;
    enable_raw_mode()?;
    execute!(tty, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(tty);
    let mut terminal = Terminal::new(backend)?;

    let mut app = AppTUI::new(db, cwd)?;

    let result = (|| -> Result<()> {
        while app.running {
            terminal.draw(|f| app.render(f))?;

            if event::poll(std::time::Duration::from_millis(100))?
                && let Event::Key(key) = event::read()?
            {
                app.handle_key(key)?;
            }
        }
        Ok(())
    })();

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result?;
    Ok(app.selected_command)
}
