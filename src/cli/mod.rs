//! Command-line interface module for Mortimer
//!
//! This module is organized into submodules:
//! - `args`: Command-line argument structures
//! - `handlers`: Command handler implementations
//! - `app`: Main CLI application structure

mod args;
mod handlers;

pub use args::*;
use handlers::*;

use crate::config::Config;
use crate::error::Result;
use crate::history::HistoryManager;
use crate::history_db::HistoryManagerDb;
use crate::search::SearchEngine;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Mortimer - Enhanced shell history manager with sensitive data redaction
#[derive(Parser)]
#[command(name = "mortimer")]
#[command(about = "Enhanced shell history manager with sensitive data redaction")]
#[command(version, long_about = None)]
pub struct Cli {
    /// Configuration file path
    #[arg(short, long, global = true)]
    pub config: Option<PathBuf>,

    /// Verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Quiet mode (suppress non-error output)
    #[arg(short, long, global = true)]
    pub quiet: bool,

    /// Disable colored output
    #[arg(long, global = true)]
    pub no_color: bool,

    /// Use database backend instead of file-based (default: auto-detect)
    #[arg(long, global = true)]
    pub use_db: bool,

    /// Force file-based backend
    #[arg(long, global = true)]
    pub use_file: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Log a command to history
    Log(LogArgs),
    /// Search command history
    Search(SearchArgs),
    /// Import history from shell files
    Import(ImportArgs),
    /// Export history to various formats
    Export(ExportArgs),
    /// Show history statistics
    Stats(StatsArgs),
    /// Clear history
    Clear(ClearArgs),
    /// Show configuration
    Config(ConfigArgs),
    /// Output commands for fuzzy finder (fzf)
    Fzf(FzfArgs),
    /// Generate shell integration scripts
    Shell(ShellArgs),
    /// Show recent commands
    Recent(RecentArgs),
    /// Show frequent commands
    Frequent(FrequentArgs),
    /// Validate redaction patterns
    Validate(ValidateArgs),
    /// Show backend status and configuration
    Status,
    /// Migrate from legacy .mhist file to database
    Migrate(MigrateArgs),
    /// Merge databases from different machines
    Merge(MergeArgs),
    /// Manage and retrieve stored tokens
    Tokens(TokensArgs),
    /// List and manage hosts
    Hosts(HostsArgs),
    /// List and manage sessions
    Sessions(SessionsArgs),
}

/// History backend type
pub(crate) enum HistoryBackend {
    File(HistoryManager),
    Database(HistoryManagerDb),
}

/// Main CLI application
pub struct CliApp {
    pub config: Config,
    pub(crate) backend: HistoryBackend,
    pub search_engine: SearchEngine,
    pub verbose: bool,
    pub quiet: bool,
    #[allow(dead_code)]
    pub no_color: bool,
}

impl CliApp {
    /// Create a new CLI application
    pub fn new(cli: &Cli) -> Result<Self> {
        // Load configuration
        let config = if let Some(config_path) = &cli.config {
            Config::load_from_path(config_path)?
        } else {
            Config::load().unwrap_or_else(|_| Config::default())
        };

        // Determine which backend to use
        let backend = if cli.use_file {
            // Explicitly use file backend
            HistoryBackend::File(HistoryManager::new(config.clone())?)
        } else if cli.use_db {
            // Explicitly use database backend
            HistoryBackend::Database(HistoryManagerDb::new(config.clone())?)
        } else {
            // Auto-detect: use database if .db file exists, otherwise use file
            let db_path = config.history_file.with_extension("db");
            if db_path.exists() {
                HistoryBackend::Database(HistoryManagerDb::new(config.clone())?)
            } else {
                HistoryBackend::File(HistoryManager::new(config.clone())?)
            }
        };

        // Initialize search engine
        let search_engine = SearchEngine::with_config(
            config.search.fuzzy_search,
            config.search.case_sensitive,
            config.search.include_directory,
            config.search.include_timestamps,
            config.search.max_results,
            config.search.highlight_matches,
        );

        Ok(Self {
            config,
            backend,
            search_engine,
            verbose: cli.verbose,
            quiet: cli.quiet,
            no_color: cli.no_color,
        })
    }

    /// Run the CLI application
    pub fn run(&mut self, command: &Commands) -> Result<()> {
        // Show backend info in verbose mode
        if self.verbose && !self.quiet {
            match &self.backend {
                HistoryBackend::File(_) => {
                    eprintln!("[verbose] Using file-based backend");
                }
                HistoryBackend::Database(_) => {
                    eprintln!("[verbose] Using SQLite database backend");
                }
            }
        }

        match command {
            Commands::Log(args) => handle_log(self, args),
            Commands::Search(args) => handle_search(self, args),
            Commands::Import(args) => handle_import(self, args),
            Commands::Export(args) => handle_export(self, args),
            Commands::Stats(args) => handle_stats(self, args),
            Commands::Clear(args) => handle_clear(self, args),
            Commands::Config(args) => handle_config(self, args),
            Commands::Fzf(args) => handle_fzf(self, args),
            Commands::Shell(args) => handle_shell(self, args),
            Commands::Recent(args) => handle_recent(self, args),
            Commands::Frequent(args) => handle_frequent(self, args),
            Commands::Validate(args) => handle_validate(self, args),
            Commands::Status => handle_status(self),
            Commands::Migrate(args) => handle_migrate(self, args),
            Commands::Merge(args) => handle_merge(self, args),
            Commands::Tokens(args) => handle_tokens(self, args),
            Commands::Hosts(args) => handle_hosts(self, args),
            Commands::Sessions(args) => handle_sessions(self, args),
        }
    }

    pub fn verbose_println(&self, message: &str) {
        if self.verbose && !self.quiet {
            eprintln!("[verbose] {}", message);
        }
    }
}

/// Main entry point for the CLI
pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let mut app = CliApp::new(&cli)?;
    app.run(&cli.command)
}
