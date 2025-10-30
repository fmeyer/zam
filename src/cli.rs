//! Command-line interface for Mortimer
//!
//! This module provides a comprehensive CLI for interacting with Mortimer,
//! including commands for logging, searching, importing, and managing
//! shell history with advanced features.

use crate::config::Config;
use crate::database::{Database, DatabaseStats, Host, Session, Token};
use crate::error::{Error, Result};
use crate::history::HistoryManager;
use crate::history_db::HistoryManagerDb;
use crate::search::{SearchEngine, SearchQuery};
use clap::{Args, Parser, Subcommand};
use std::io::{self, Write};
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
    /// Migrate from legacy .mhist file to database
    Migrate(MigrateArgs),
    /// Merge databases from different machines
    Merge(MergeArgs),
    /// Manage and retrieve stored tokens
    Tokens(TokensArgs),
    /// List and manage hosts
    Hosts(HostsArgs),
    /// List and manage session
    Session(SessionArgs),
}

#[derive(Args)]
pub struct LogArgs {
    /// Command to log
    #[arg(value_name = "COMMAND")]
    pub command: String,

    /// Timestamp in Unix format (optional)
    #[arg(short = 'T', long)]
    pub timestamp: Option<i64>,

    /// Directory where command was executed (optional)
    #[arg(short = 'D', long)]
    pub directory: Option<String>,

    /// Skip redaction for this command
    #[arg(long)]
    pub no_redact: bool,
}

#[derive(Args)]
pub struct SearchArgs {
    /// Search term
    #[arg(value_name = "TERM")]
    pub term: String,

    /// Filter by directory
    #[arg(short = 'D', long)]
    pub directory: Option<String>,

    /// Use exact matching (disable fuzzy search)
    #[arg(short = 'E', long)]
    pub exact: bool,

    /// Case-sensitive search
    #[arg(short = 'C', long)]
    pub case_sensitive: bool,

    /// Use regex matching
    #[arg(short = 'R', long)]
    pub regex: bool,

    /// Search only redacted commands
    #[arg(long)]
    pub redacted_only: bool,

    /// Maximum number of results
    #[arg(short = 'L', long, default_value = "50")]
    pub limit: usize,

    /// Show timestamps
    #[arg(short = 'T', long)]
    pub timestamps: bool,

    /// Show directories
    #[arg(long)]
    pub show_dirs: bool,

    /// Search within specific time range (format: YYYY-MM-DD)
    #[arg(long)]
    pub since: Option<String>,

    /// Search before specific date (format: YYYY-MM-DD)
    #[arg(long)]
    pub before: Option<String>,
}

#[derive(Args)]
pub struct ImportArgs {
    /// Shell type to import from
    #[arg(value_enum, default_value = "zsh")]
    pub shell: ShellType,

    /// Path to history file (optional, auto-detected if not provided)
    #[arg(short = 'F', long)]
    pub file: Option<PathBuf>,

    /// Dry run - show what would be imported without actually importing
    #[arg(long)]
    pub dry_run: bool,

    /// Skip deduplication
    #[arg(long)]
    pub no_dedup: bool,

    /// Import entries from last N days only
    #[arg(long)]
    pub days: Option<u32>,

    /// Show progress during import
    #[arg(long)]
    pub progress: bool,
}

#[derive(Args)]
pub struct ExportArgs {
    /// Export format
    #[arg(value_enum, default_value = "json")]
    pub format: ExportFormat,

    /// Output file (stdout if not specified)
    #[arg(short = 'O', long)]
    pub output: Option<PathBuf>,

    /// Include redacted commands
    #[arg(long)]
    pub include_redacted: bool,

    /// Include original commands (if available)
    #[arg(long)]
    pub include_original: bool,

    /// Export from specific directory only
    #[arg(short = 'D', long)]
    pub directory: Option<String>,

    /// Export entries from last N days only
    #[arg(long)]
    pub days: Option<u32>,
}

#[derive(Args)]
pub struct StatsArgs {
    /// Show detailed statistics
    #[arg(short = 'D', long)]
    pub detailed: bool,

    /// Show redaction statistics
    #[arg(long)]
    pub redaction: bool,

    /// Show directory statistics
    #[arg(long)]
    pub directories: bool,

    /// Show time-based statistics
    #[arg(long)]
    pub time_stats: bool,
}

#[derive(Args)]
pub struct ClearArgs {
    /// Confirm deletion without prompting
    #[arg(short = 'F', long)]
    pub force: bool,

    /// Keep last N entries
    #[arg(long)]
    pub keep: Option<usize>,

    /// Clear entries older than N days
    #[arg(long)]
    pub older_than: Option<u32>,
}

#[derive(Args)]
pub struct ConfigArgs {
    /// Show current configuration
    #[arg(long)]
    pub show: bool,

    /// Initialize configuration file with defaults
    #[arg(long)]
    pub init: bool,

    /// Validate configuration file
    #[arg(long)]
    pub validate: bool,

    /// Set configuration value (format: key=value)
    #[arg(long)]
    pub set: Option<String>,

    /// Get configuration value
    #[arg(long)]
    pub get: Option<String>,
}

#[derive(Args)]
pub struct FzfArgs {
    /// Show unique commands only
    #[arg(short = 'U', long)]
    pub unique: bool,

    /// Filter by directory
    #[arg(short = 'D', long)]
    pub directory: Option<String>,

    /// Maximum number of results
    #[arg(short = 'L', long, default_value = "1000")]
    pub limit: usize,

    /// Reverse order (oldest first)
    #[arg(short = 'R', long)]
    pub reverse: bool,
}

#[derive(Args)]
pub struct ShellArgs {
    /// Shell type to generate integration for
    #[arg(value_enum)]
    pub shell: ShellType,

    /// Output file (stdout if not specified)
    #[arg(short = 'O', long)]
    pub output: Option<PathBuf>,

    /// Include custom key bindings
    #[arg(long)]
    pub custom_bindings: bool,
}

#[derive(Args)]
pub struct RecentArgs {
    /// Number of recent commands to show
    #[arg(short = 'n', long, default_value = "20")]
    pub count: usize,

    /// Filter by directory
    #[arg(short = 'D', long)]
    pub directory: Option<String>,

    /// Show timestamps
    #[arg(short = 'T', long)]
    pub timestamps: bool,
}

#[derive(Args)]
pub struct FrequentArgs {
    /// Number of frequent commands to show
    #[arg(short = 'n', long, default_value = "20")]
    pub count: usize,

    /// Show directories instead of commands
    #[arg(long)]
    pub directories: bool,

    /// Show usage counts
    #[arg(long)]
    pub counts: bool,
}

#[derive(Args)]
pub struct ValidateArgs {
    /// Pattern to validate
    #[arg(value_name = "PATTERN")]
    pub pattern: String,

    /// Test the pattern against example text
    #[arg(short = 'T', long)]
    pub test: Option<String>,
}

#[derive(clap::ValueEnum, Clone)]
pub enum ShellType {
    Zsh,
    Bash,
    Fish,
}

#[derive(Args)]
pub struct MigrateArgs {
    /// Path to legacy .mhist file
    #[arg(value_name = "MHIST_FILE")]
    pub mhist_file: PathBuf,

    /// Show what would be migrated without actually migrating
    #[arg(long)]
    pub dry_run: bool,

    /// Show progress during migration
    #[arg(long)]
    pub progress: bool,
}

#[derive(Args)]
pub struct MergeArgs {
    /// Path to database file to merge from
    #[arg(value_name = "DB_FILE")]
    pub db_file: PathBuf,

    /// Show what would be merged without actually merging
    #[arg(long)]
    pub dry_run: bool,

    /// Show progress during merge
    #[arg(long)]
    pub progress: bool,
}

#[derive(Args)]
pub struct TokensArgs {
    /// Filter by session ID
    #[arg(short = 'S', long)]
    pub session: Option<String>,

    /// Filter by directory
    #[arg(short = 'D', long)]
    pub directory: Option<String>,

    /// Filter by command ID
    #[arg(short = 'C', long)]
    pub command_id: Option<i64>,

    /// Show token values (use with caution!)
    #[arg(long)]
    pub show_values: bool,

    /// Export tokens to file
    #[arg(short = 'O', long)]
    pub output: Option<PathBuf>,
}

#[derive(Args)]
pub struct HostsArgs {
    /// List all hosts
    #[arg(short = 'L', long)]
    pub list: bool,

    /// Show sessions for a specific host
    #[arg(short = 'S', long)]
    pub show_sessions: Option<i64>,

    /// Show detailed information
    #[arg(short = 'D', long)]
    pub detailed: bool,
}

#[derive(Args)]
pub struct SessionsArgs {
    /// Filter by host ID
    #[arg(short = 'H', long)]
    pub host_id: Option<i64>,

    /// Show active sessions only
    #[arg(short = 'A', long)]
    pub active: bool,

    /// Show commands in session
    #[arg(short = 'C', long)]
    pub show_commands: Option<String>,

    /// Show detailed information
    #[arg(short = 'D', long)]
    pub detailed: bool,
}

#[derive(clap::ValueEnum, Clone)]
pub enum ExportFormat {
    Json,
    Csv,
    Tsv,
    Plain,
}

/// CLI application handler
enum HistoryBackend {
    File(HistoryManager),
    Database(HistoryManagerDb),
}

pub struct CliApp {
    config: Config,
    backend: HistoryBackend,
    search_engine: SearchEngine,
    verbose: bool,
    quiet: bool,
    #[allow(dead_code)]
    no_color: bool,
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
        match command {
            Commands::Log(args) => self.handle_log(args),
            Commands::Search(args) => self.handle_search(args),
            Commands::Import(args) => self.handle_import(args),
            Commands::Export(args) => self.handle_export(args),
            Commands::Stats(args) => self.handle_stats(args),
            Commands::Clear(args) => self.handle_clear(args),
            Commands::Config(args) => self.handle_config(args),
            Commands::Fzf(args) => self.handle_fzf(args),
            Commands::Shell(args) => self.handle_shell(args),
            Commands::Recent(args) => self.handle_recent(args),
            Commands::Frequent(args) => self.handle_frequent(args),
            Commands::Validate(args) => self.handle_validate(args),
            Commands::Migrate(args) => self.handle_migrate(args),
            Commands::Merge(args) => self.handle_merge(args),
            Commands::Tokens(args) => self.handle_tokens(args),
            Commands::Hosts(args) => self.handle_hosts(args),
            Commands::Sessions(args) => self.handle_sessions(args),
        }
    }

    fn handle_log(&mut self, args: &LogArgs) -> Result<()> {
        if !self.quiet {
            self.verbose_println(&format!("Logging command: {}", args.command));
        }

        // Handle timestamp
        let timestamp = if let Some(ts) = args.timestamp {
            Some(chrono::DateTime::from_timestamp(ts, 0).ok_or_else(|| {
                Error::InvalidTimestamp {
                    timestamp: ts.to_string(),
                }
            })?)
        } else {
            None
        };

        // Log the command based on backend
        match &mut self.backend {
            HistoryBackend::File(mgr) => {
                if let Some(timestamp) = timestamp {
                    mgr.log_command_with_timestamp(&args.command, Some(timestamp))?;
                } else {
                    mgr.log_command(&args.command)?;
                }
            }
            HistoryBackend::Database(mgr) => {
                mgr.log_command_with_timestamp(&args.command, timestamp, None)?;
            }
        }

        if !self.quiet {
            println!("Command logged successfully");
        }

        Ok(())
    }

    fn handle_search(&mut self, args: &SearchArgs) -> Result<()> {
        // Get entries based on backend
        let entries = match &self.backend {
            HistoryBackend::File(mgr) => mgr.get_entries()?,
            HistoryBackend::Database(mgr) => {
                // For database, use direct search if no complex filters
                if args.since.is_none() && args.before.is_none() && !args.regex && !args.exact {
                    let db_results = mgr.search(
                        &args.term,
                        args.directory.as_deref(),
                        None,
                        Some(args.limit),
                    )?;

                    // Display results
                    for result in &db_results {
                        let mut output = String::new();

                        if args.timestamps {
                            output.push_str(&format!(
                                "{} ",
                                result.timestamp.format("%Y-%m-%d %H:%M:%S")
                            ));
                        }

                        if args.show_dirs {
                            output.push_str(&format!("{} ", result.directory));
                        }

                        output.push_str(&result.command);
                        println!("{}", output);
                    }

                    if !self.quiet {
                        println!("\nFound {} results", db_results.len());
                    }
                    return Ok(());
                }

                // Otherwise, get all and use search engine
                mgr.get_all_commands()?
                    .into_iter()
                    .map(|cmd| crate::history::HistoryEntry {
                        command: cmd.command,
                        timestamp: cmd.timestamp,
                        directory: cmd.directory,
                        redacted: cmd.redacted,
                        original: None,
                    })
                    .collect()
            }
        };

        // Build search query
        let mut query = SearchQuery::new(args.term.clone());

        if let Some(dir) = &args.directory {
            query = query.with_directory(dir.clone());
        }

        if args.exact {
            query.fuzzy = false;
        }

        if args.case_sensitive {
            query.case_sensitive = true;
        }

        if args.regex {
            query = query.regex();
        }

        if args.redacted_only {
            query = query.redacted_only();
        }

        query = query.limit(args.limit);

        // Parse time filters
        if let Some(since_str) = &args.since {
            let since = chrono::NaiveDate::parse_from_str(since_str, "%Y-%m-%d")
                .map_err(|_| Error::InvalidTimestamp {
                    timestamp: since_str.clone(),
                })?
                .and_hms_opt(0, 0, 0)
                .unwrap()
                .and_utc();

            let end = if let Some(before_str) = &args.before {
                chrono::NaiveDate::parse_from_str(before_str, "%Y-%m-%d")
                    .map_err(|_| Error::InvalidTimestamp {
                        timestamp: before_str.clone(),
                    })?
                    .and_hms_opt(23, 59, 59)
                    .unwrap()
                    .and_utc()
            } else {
                chrono::Utc::now()
            };

            query = query.with_time_range(since, end);
        }

        // Perform search
        let results = self.search_engine.search_with_query(&entries, &query)?;

        if results.is_empty() {
            if !self.quiet {
                println!("No results found for '{}'", args.term);
            }
            return Ok(());
        }

        // Display results
        for (i, result) in results.iter().enumerate() {
            if i >= args.limit {
                break;
            }

            let mut output = String::new();

            if args.timestamps {
                output.push_str(&format!("{} ", result.entry.formatted_timestamp()));
            }

            if args.show_dirs {
                output.push_str(&format!("{} ", result.entry.directory));
            }

            if let Some(ref highlighted) = result.highlighted {
                output.push_str(highlighted);
            } else {
                output.push_str(&result.entry.command);
            }

            println!("{}", output);
        }

        if !self.quiet {
            println!("\nFound {} results", results.len());
        }

        Ok(())
    }

    fn handle_import(&mut self, args: &ImportArgs) -> Result<()> {
        let shell_name = match args.shell {
            ShellType::Zsh => "zsh",
            ShellType::Bash => "bash",
            ShellType::Fish => "fish",
        };

        if !self.quiet {
            println!("Importing {} history...", shell_name);
        }

        if args.dry_run {
            println!("DRY RUN: Would import from {} history", shell_name);
            return Ok(());
        }

        let imported_count = match &mut self.backend {
            HistoryBackend::File(mgr) => mgr.import_from_shell(shell_name, args.file.clone())?,
            HistoryBackend::Database(mgr) => match args.shell {
                ShellType::Zsh => mgr.import_from_zsh(args.file.clone())?,
                ShellType::Bash => mgr.import_from_bash(args.file.clone())?,
                ShellType::Fish => mgr.import_from_fish(args.file.clone())?,
            },
        };

        if !self.quiet {
            println!(
                "Successfully imported {} commands from {} history",
                imported_count, shell_name
            );
        }

        Ok(())
    }

    fn handle_export(&mut self, args: &ExportArgs) -> Result<()> {
        let entries = match &self.backend {
            HistoryBackend::File(mgr) => mgr.get_entries()?,
            HistoryBackend::Database(mgr) => mgr
                .get_all_commands()?
                .into_iter()
                .map(|cmd| crate::history::HistoryEntry {
                    command: cmd.command,
                    timestamp: cmd.timestamp,
                    directory: cmd.directory,
                    redacted: cmd.redacted,
                    original: None,
                })
                .collect(),
        };

        // Filter entries if needed
        let filtered_entries: Vec<_> = entries
            .into_iter()
            .filter(|entry| {
                if let Some(dir) = &args.directory {
                    if !entry.directory.contains(dir) {
                        return false;
                    }
                }

                if let Some(days) = args.days {
                    let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);
                    if entry.timestamp < cutoff {
                        return false;
                    }
                }

                true
            })
            .collect();

        let output = match args.format {
            ExportFormat::Json => serde_json::to_string_pretty(&filtered_entries)?,
            ExportFormat::Csv => {
                let mut output = String::from("timestamp,directory,command\n");
                for entry in &filtered_entries {
                    output.push_str(&format!(
                        "{},{},{}\n",
                        entry.timestamp.to_rfc3339(),
                        entry.directory,
                        entry.command.replace(",", "\\,")
                    ));
                }
                output
            }
            ExportFormat::Tsv => {
                let mut output = String::from("timestamp\tdirectory\tcommand\n");
                for entry in &filtered_entries {
                    output.push_str(&format!(
                        "{}\t{}\t{}\n",
                        entry.timestamp.to_rfc3339(),
                        entry.directory,
                        entry.command
                    ));
                }
                output
            }
            ExportFormat::Plain => filtered_entries
                .iter()
                .map(|entry| entry.command.clone())
                .collect::<Vec<_>>()
                .join("\n"),
        };

        if let Some(output_file) = &args.output {
            std::fs::write(output_file, output)?;
            if !self.quiet {
                println!(
                    "Exported {} entries to {}",
                    filtered_entries.len(),
                    output_file.display()
                );
            }
        } else {
            print!("{}", output);
        }

        Ok(())
    }

    fn handle_stats(&mut self, args: &StatsArgs) -> Result<()> {
        match &self.backend {
            HistoryBackend::File(mgr) => {
                let stats = mgr.get_stats();

                println!("History Statistics");
                println!("==================");
                println!("Total entries: {}", stats.total_entries);
                println!("Unique commands: {}", stats.unique_commands);
                println!("Redacted entries: {}", stats.redacted_entries);
                println!("Duplicates filtered: {}", stats.duplicates_filtered);

                if args.redaction {
                    println!("\nRedaction Statistics");
                    println!("===================");
                    println!(
                        "Total commands processed: {}",
                        stats.redaction_stats.total_commands
                    );
                    println!(
                        "Commands redacted: {}",
                        stats.redaction_stats.redacted_commands
                    );
                    println!(
                        "Environment variables redacted: {}",
                        stats.redaction_stats.env_vars_redacted
                    );

                    if !stats.redaction_stats.patterns_matched.is_empty() {
                        println!("\nPatterns matched:");
                        for (pattern, count) in &stats.redaction_stats.patterns_matched {
                            println!("  {}: {}", pattern, count);
                        }
                    }
                }

                if args.directories {
                    println!("\nDirectory Statistics");
                    println!("===================");
                    let mut dirs: Vec<_> = stats.common_directories.iter().collect();
                    dirs.sort_by(|a, b| b.1.cmp(a.1));
                    for (dir, count) in dirs.iter().take(10) {
                        println!("  {}: {}", dir, count);
                    }
                }
            }
            HistoryBackend::Database(mgr) => {
                let stats = mgr.get_stats()?;

                println!("History Statistics (Database)");
                println!("==============================");
                println!("Total commands: {}", stats.total_commands);
                println!("Total sessions: {}", stats.total_sessions);
                println!("Total hosts: {}", stats.total_hosts);
                println!("Redacted commands: {}", stats.redacted_commands);
                println!("Stored tokens: {}", stats.stored_tokens);

                if let Some(oldest) = stats.oldest_entry {
                    println!("Oldest entry: {}", oldest.format("%Y-%m-%d %H:%M:%S"));
                }
                if let Some(newest) = stats.newest_entry {
                    println!("Newest entry: {}", newest.format("%Y-%m-%d %H:%M:%S"));
                }
            }
        }

        Ok(())
    }

    fn handle_clear(&mut self, args: &ClearArgs) -> Result<()> {
        if !args.force {
            print!("Are you sure you want to clear the history? (y/N): ");
            io::stdout().flush()?;
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            if !input.trim().eq_ignore_ascii_case("y") {
                println!("Aborted");
                return Ok(());
            }
        }

        match &self.backend {
            HistoryBackend::File(mgr) => mgr.clear()?,
            HistoryBackend::Database(mgr) => mgr.clear()?,
        }

        if !self.quiet {
            println!("History cleared successfully");
        }

        Ok(())
    }

    fn handle_config(&mut self, args: &ConfigArgs) -> Result<()> {
        if args.show {
            let config_json = serde_json::to_string_pretty(&self.config)?;
            println!("{}", config_json);
        } else if args.init {
            let config_path = Config::default_config_path()?;
            let config = Config::default();
            config.save_to_path(&config_path)?;
            println!("Configuration initialized at {}", config_path.display());
        } else if args.validate {
            match self.config.validate() {
                Ok(_) => println!("Configuration is valid"),
                Err(e) => println!("Configuration validation failed: {}", e),
            }
        } else {
            println!("Use --show, --init, or --validate");
        }

        Ok(())
    }

    fn handle_fzf(&mut self, args: &FzfArgs) -> Result<()> {
        let mut commands = if args.unique {
            self.history_manager.get_unique_commands()?
        } else {
            self.history_manager
                .get_entries()?
                .into_iter()
                .map(|entry| entry.command)
                .collect()
        };

        if args.reverse {
            commands.reverse();
        }

        commands.truncate(args.limit);

        for command in commands {
            println!("{}", command);
        }

        Ok(())
    }

    fn handle_shell(&mut self, args: &ShellArgs) -> Result<()> {
        let shell_script = match args.shell {
            ShellType::Zsh => self.generate_zsh_integration(),
            ShellType::Bash => self.generate_bash_integration(),
            ShellType::Fish => self.generate_fish_integration(),
        };

        if let Some(output_file) = &args.output {
            std::fs::write(output_file, shell_script)?;
            println!("Shell integration written to {}", output_file.display());
        } else {
            print!("{}", shell_script);
        }

        Ok(())
    }

    fn handle_recent(&mut self, args: &RecentArgs) -> Result<()> {
        let entries = match &self.backend {
            HistoryBackend::File(mgr) => {
                let mut entries = mgr.get_entries()?;
                entries.reverse();
                entries.truncate(args.count);
                entries
            }
            HistoryBackend::Database(mgr) => mgr
                .get_recent(args.count)?
                .into_iter()
                .map(|cmd| crate::history::HistoryEntry {
                    command: cmd.command,
                    timestamp: cmd.timestamp,
                    directory: cmd.directory,
                    redacted: cmd.redacted,
                    original: None,
                })
                .collect(),
        };

        for entry in entries {
            if args.timestamps {
                println!("{} {}", entry.formatted_timestamp(), entry.command);
            } else {
                println!("{}", entry.command);
            }
        }

        Ok(())
    }

    fn handle_fzf(&mut self, args: &FzfArgs) -> Result<()> {
        let entries = match &self.backend {
            HistoryBackend::File(mgr) => mgr.get_entries()?,
            HistoryBackend::Database(mgr) => mgr
                .get_all_commands()?
                .into_iter()
                .map(|cmd| crate::history::HistoryEntry {
                    command: cmd.command,
                    timestamp: cmd.timestamp,
                    directory: cmd.directory,
                    redacted: cmd.redacted,
                    original: None,
                })
                .collect(),
        };

        // Filter by directory if specified
        if let Some(dir) = &args.directory {
            entries.retain(|entry| entry.directory.contains(dir));
        }

        // Sort by timestamp (most recent first)
        entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        // Take the requested number of entries
        entries.truncate(args.count);

        for entry in entries {
            let mut output = String::new();

            if args.timestamps {
                output.push_str(&format!("{} ", entry.formatted_timestamp()));
            }

            output.push_str(&entry.command);
            println!("{}", output);
        }

        Ok(())
    }

    fn handle_frequent(&mut self, args: &FrequentArgs) -> Result<()> {
        let entries = match &self.backend {
            HistoryBackend::File(mgr) => mgr.get_entries()?,
            HistoryBackend::Database(mgr) => mgr
                .get_all_commands()?
                .into_iter()
                .map(|cmd| crate::history::HistoryEntry {
                    command: cmd.command,
                    timestamp: cmd.timestamp,
                    directory: cmd.directory,
                    redacted: cmd.redacted,
                    original: None,
                })
                .collect(),
        };

        if args.directories {
            let frequent_dirs = self.search_engine.get_frequent_directories(&entries)?;
            for (dir, count) in frequent_dirs.iter().take(args.count) {
                if args.counts {
                    println!("{}: {}", dir, count);
                } else {
                    println!("{}", dir);
                }
            }
        } else {
            let frequent_commands = self.search_engine.get_frequent_commands(&entries)?;
            for (command, count) in frequent_commands.iter().take(args.count) {
                if args.counts {
                    println!("{}: {}", command, count);
                } else {
                    println!("{}", command);
                }
            }
        }

        Ok(())
    }

    fn handle_validate(&mut self, args: &ValidateArgs) -> Result<()> {
        // Try to compile the regex
        match regex::Regex::new(&args.pattern) {
            Ok(regex) => {
                println!("Pattern '{}' is valid", args.pattern);

                if let Some(test_text) = &args.test {
                    if regex.is_match(test_text) {
                        println!("Pattern matches test text");
                        for mat in regex.find_iter(test_text) {
                            println!(
                                "  Match: '{}' at position {}-{}",
                                mat.as_str(),
                                mat.start(),
                                mat.end()
                            );
                        }
                    } else {
                        println!("Pattern does not match test text");
                    }
                }
            }
            Err(e) => {
                println!("Pattern '{}' is invalid: {}", args.pattern, e);
            }
        }

        Ok(())
    }

    fn generate_zsh_integration(&self) -> String {
        r#"# Mortimer Zsh Integration
# Add this to your ~/.zshrc

# Custom history manager function
log_command() {
    mortimer log "$1"
}

# Hook to log commands before execution
autoload -Uz add-zsh-hook
add-zsh-hook preexec log_command

# Custom history search with Ctrl-R for fuzzy search
mortimer-history-widget() {
    BUFFER=$(mortimer fzf | fzf --height 50% --reverse --tac 2>/dev/null)
    CURSOR=$#BUFFER
    zle reset-prompt
}

# Custom history search with Ctrl-E for exact match
mortimer-history-exact-widget() {
    BUFFER=$(mortimer fzf | fzf -e -i --height 50% --reverse --tac 2>/dev/null)
    CURSOR=$#BUFFER
    zle reset-prompt
}

zle -N mortimer-history-widget
zle -N mortimer-history-exact-widget

# Replace default Ctrl-R with the custom widget
bindkey '^R' mortimer-history-widget
bindkey '^E' mortimer-history-exact-widget
"#
        .to_string()
    }

    fn generate_bash_integration(&self) -> String {
        r#"# Mortimer Bash Integration
# Add this to your ~/.bashrc

# Function to log commands
log_command() {
    mortimer log "$1"
}

# Hook to log commands after execution
export PROMPT_COMMAND="log_command \"\$(history 1 | sed 's/^[ ]*[0-9]*[ ]*//')\"${PROMPT_COMMAND:+;$PROMPT_COMMAND}"

# Custom history search function
mortimer_search() {
    local selected
    selected=$(mortimer fzf | fzf --height 50% --reverse --tac 2>/dev/null)
    if [ -n "$selected" ]; then
        READLINE_LINE="$selected"
        READLINE_POINT=${#READLINE_LINE}
    fi
}

# Bind Ctrl-R to custom search
bind -x '"\C-r": mortimer_search'
"#.to_string()
    }

    fn generate_fish_integration(&self) -> String {
        r#"# Mortimer Fish Integration
# Add this to your ~/.config/fish/config.fish

# Function to log commands
function log_command --on-event fish_preexec
    mortimer log "$argv[1]"
end

# Custom history search function
function mortimer_search
    set -l selected (mortimer fzf | fzf --height 50% --reverse --tac 2>/dev/null)
    if test -n "$selected"
        commandline -r "$selected"
    end
    commandline -f repaint
end

# Bind Ctrl-R to custom search
bind \cr mortimer_search
"#
        .to_string()
    }

    fn verbose_println(&self, message: &str) {
        if self.verbose && !self.quiet {
            eprintln!("[verbose] {}", message);
        }
    }

    fn handle_migrate(&mut self, args: &MigrateArgs) -> Result<()> {
        let mgr = match &mut self.backend {
            HistoryBackend::Database(mgr) => mgr,
            HistoryBackend::File(_) => {
                return Err(Error::custom(
                    "Migration requires database backend. Use --use-db flag.",
                ));
            }
        };

        if !self.quiet {
            println!("Migrating from .mhist file: {}", args.mhist_file.display());
        }

        if args.dry_run {
            println!("DRY RUN: Would migrate from {}", args.mhist_file.display());
            return Ok(());
        }

        let count = mgr.import_from_mhist(&args.mhist_file)?;

        if !self.quiet {
            println!("Successfully migrated {} commands", count);
        }

        Ok(())
    }

    fn handle_merge(&mut self, args: &MergeArgs) -> Result<()> {
        let mgr = match &mut self.backend {
            HistoryBackend::Database(mgr) => mgr,
            HistoryBackend::File(_) => {
                return Err(Error::custom(
                    "Merge requires database backend. Use --use-db flag.",
                ));
            }
        };

        if !self.quiet {
            println!("Merging database from: {}", args.db_file.display());
        }

        if args.dry_run {
            println!("DRY RUN: Would merge from {}", args.db_file.display());
            return Ok(());
        }

        let count = mgr.merge_from_database(&args.db_file)?;

        if !self.quiet {
            println!("Successfully merged {} commands", count);
        }

        Ok(())
    }

    fn handle_tokens(&mut self, args: &TokensArgs) -> Result<()> {
        let mgr = match &self.backend {
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
            if !self.quiet {
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

        if !self.quiet {
            println!("Total tokens: {}", tokens.len());
        }

        Ok(())
    }

    fn handle_hosts(&mut self, args: &HostsArgs) -> Result<()> {
        let mgr = match &self.backend {
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

    fn handle_sessions(&mut self, args: &SessionsArgs) -> Result<()> {
        let mgr = match &self.backend {
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
}

/// Main entry point for the CLI
pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let mut app = CliApp::new(&cli)?;
    app.run(&cli.command)
}
