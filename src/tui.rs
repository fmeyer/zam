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
use std::io;

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Commands,
    Aliases,
    Hosts,
    Sessions,
    Tokens,
}

const TABS: [Tab; 5] = [
    Tab::Commands,
    Tab::Aliases,
    Tab::Hosts,
    Tab::Sessions,
    Tab::Tokens,
];

impl Tab {
    fn title(self) -> &'static str {
        match self {
            Tab::Commands => "Commands",
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
}

struct AppTUI<'a> {
    db: &'a Database,
    tab: Tab,
    mode: Mode,

    // Data
    commands: Vec<CommandEntry>,
    aliases: Vec<Alias>,
    hosts: Vec<Host>,
    sessions: Vec<Session>,
    tokens: Vec<Token>,

    // Table state per tab
    table_state: TableState,
    row_count: usize,

    // Filter
    filter: String,

    // Confirm delete
    confirm_msg: String,

    // Status
    status: Option<String>,
    show_values: bool,
    running: bool,
}

impl<'a> AppTUI<'a> {
    fn new(db: &'a Database) -> Result<Self> {
        let mut app = Self {
            db,
            tab: Tab::Commands,
            mode: Mode::Normal,
            commands: Vec::new(),
            aliases: Vec::new(),
            hosts: Vec::new(),
            sessions: Vec::new(),
            tokens: Vec::new(),
            table_state: TableState::default(),
            row_count: 0,
            filter: String::new(),
            confirm_msg: String::new(),
            status: None,
            show_values: false,
            running: true,
        };
        app.load_tab()?;
        Ok(app)
    }

    fn load_tab(&mut self) -> Result<()> {
        self.table_state = TableState::default();
        match self.tab {
            Tab::Commands => {
                self.commands = self.db.get_all_commands()?;
                self.row_count = self.commands.len();
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
                    format!("Delete command \"{}\"?", preview)
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
                    self.status = Some("Command deleted".into());
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
                    self.db.delete_session(&s.id.to_string())?;
                    self.status = Some("Session deleted".into());
                }
            }
            Tab::Tokens => {
                if let Some(t) = self.tokens.get(idx) {
                    self.db.delete_token(t.id)?;
                    self.status = Some("Token deleted".into());
                }
            }
        }
        self.mode = Mode::Normal;
        self.load_tab()
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        match self.mode {
            Mode::Confirm => match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => self.confirm_delete()?,
                _ => self.mode = Mode::Normal,
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
                    self.tab = Tab::Commands;
                    self.filter.clear();
                    self.load_tab()?;
                }
                KeyCode::Char('2') => {
                    self.tab = Tab::Aliases;
                    self.filter.clear();
                    self.load_tab()?;
                }
                KeyCode::Char('3') => {
                    self.tab = Tab::Hosts;
                    self.filter.clear();
                    self.load_tab()?;
                }
                KeyCode::Char('4') => {
                    self.tab = Tab::Sessions;
                    self.filter.clear();
                    self.load_tab()?;
                }
                KeyCode::Char('5') => {
                    self.tab = Tab::Tokens;
                    self.filter.clear();
                    self.load_tab()?;
                }
                KeyCode::Up | KeyCode::Char('k') => self.select_prev(),
                KeyCode::Down | KeyCode::Char('j') => self.select_next(),
                KeyCode::Char('/') => {
                    self.filter.clear();
                    self.mode = Mode::Filter;
                }
                KeyCode::Char('d') | KeyCode::Delete => self.request_delete(),
                KeyCode::Char('v') if self.tab == Tab::Tokens => {
                    self.show_values = !self.show_values;
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
                Constraint::Min(5),   // Table
                Constraint::Length(1), // Status
            ])
            .split(frame.area());

        self.render_tabs(frame, chunks[0]);
        self.render_table(frame, chunks[1]);
        self.render_status(frame, chunks[2]);

        if self.mode == Mode::Confirm {
            self.render_confirm(frame, frame.area());
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
        text.to_lowercase()
            .contains(&self.filter.to_lowercase())
    }

    fn render_commands(&mut self, frame: &mut Frame, area: Rect) {
        let header = Row::new(vec!["ID", "Timestamp", "Command", "Directory", "R"])
            .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));

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
        )
        .header(header)
        .block(Block::default().borders(Borders::ALL).title("Commands"))
        .row_highlight_style(Style::default().bg(Color::DarkGray));

        frame.render_stateful_widget(table, area, &mut self.table_state);
    }

    fn render_aliases(&mut self, frame: &mut Frame, area: Rect) {
        let header = Row::new(vec!["Alias", "Command", "Description", "Updated"])
            .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));

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
        let header = Row::new(vec!["ID", "Hostname", "Created"])
            .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));

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
        let header = Row::new(vec!["ID", "Host", "Started", "Ended"])
            .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));

        let rows: Vec<Row> = self
            .sessions
            .iter()
            .filter(|s| self.matches_filter(&s.id.to_string()))
            .map(|s| {
                let ended = s
                    .ended_at
                    .map(|e| e.format("%Y-%m-%d %H:%M").to_string())
                    .unwrap_or_else(|| "active".into());
                Row::new(vec![
                    Cell::from(truncate(&s.id.to_string(), 12)),
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
        let header = Row::new(vec!["ID", "Cmd", "Type", "Placeholder", "Value", "Created"])
            .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));

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
            Mode::Confirm => String::new(),
            Mode::Normal => {
                if let Some(ref msg) = self.status {
                    format!(" {} | q=quit Tab=switch /=filter d=delete", msg)
                } else {
                    " q=quit Tab=switch /=filter d=delete ?=help".into()
                }
            }
        };

        let style = match self.mode {
            Mode::Filter => Style::default().fg(Color::Yellow),
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

/// Run the interactive TUI for browsing database entities
pub fn run_tui(db: &Database) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = AppTUI::new(db)?;

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

    result
}
