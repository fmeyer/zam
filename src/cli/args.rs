//! Command-line argument structures for zam

use clap::Args;
use std::path::PathBuf;

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

    /// Exit code of the command (optional)
    #[arg(short = 'E', long)]
    pub exit_code: Option<i32>,

    /// Use a static session ID (e.g. for non-interactive tools like Claude Code)
    #[arg(short = 'S', long)]
    pub session_id: Option<String>,
}

#[derive(Args)]
pub struct EndSessionArgs {
    /// Session ID to close
    #[arg(value_name = "SESSION_ID")]
    pub session_id: String,
}

#[derive(Args)]
pub struct AuthArgs {
    /// 1Password item name to load secrets from
    #[arg(value_name = "ITEM")]
    pub item: Option<String>,

    /// List keys loaded for the current session
    #[arg(short = 'L', long)]
    pub list: bool,

    /// Output export statements (used by zam-auth wrapper)
    #[arg(long)]
    pub export: bool,

    /// Clear all session secrets (outputs unset lines)
    #[arg(long)]
    pub clear: bool,

    /// Store a secret in the 1Password item (format: KEY:VALUE)
    #[arg(long, value_name = "KEY:VALUE")]
    pub set: Option<String>,

    /// Session ID to associate secrets with
    #[arg(short = 'S', long)]
    pub session_id: Option<String>,
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
    /// Shell type
    #[arg(value_enum)]
    pub shell: ShellType,

    /// Output file (optional, prints to stdout if not specified)
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
    /// Number of frequent items to show
    #[arg(short = 'n', long, default_value = "10")]
    pub count: usize,

    /// Show frequent directories instead of commands
    #[arg(long)]
    pub directories: bool,

    /// Show counts alongside items
    #[arg(long)]
    pub counts: bool,
}

#[derive(Args)]
pub struct ValidateArgs {
    /// Redaction pattern to validate
    #[arg(value_name = "PATTERN")]
    pub pattern: String,

    /// Test string to validate against the pattern
    #[arg(short = 't', long)]
    pub test: Option<String>,
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

#[derive(Args)]
pub struct AliasArgs {
    #[command(subcommand)]
    pub command: AliasCommands,
}

#[derive(clap::Subcommand)]
pub enum AliasCommands {
    /// Add a new alias
    Add(AliasAddArgs),
    /// Update an existing alias
    Update(AliasUpdateArgs),
    /// Remove an alias
    Remove(AliasRemoveArgs),
    /// List all aliases
    List(AliasListArgs),
    /// Export aliases as a shell script
    Export(AliasExportArgs),
    /// Sync aliases from shell (reads alias output from stdin)
    Sync,
}

#[derive(Args)]
pub struct AliasAddArgs {
    /// Alias name
    pub name: String,
    /// Command the alias expands to
    pub command: String,
    /// Description of the alias
    pub description: String,
}

#[derive(Args)]
pub struct AliasUpdateArgs {
    /// Alias name
    pub name: String,
    /// New command the alias expands to
    pub command: String,
    /// New description (optional)
    #[arg(short, long)]
    pub description: Option<String>,
}

#[derive(Args)]
pub struct AliasRemoveArgs {
    /// Alias name to remove
    pub name: String,
}

#[derive(Args)]
pub struct AliasListArgs {
    /// Output in shell eval-ready format (alias name='cmd')
    #[arg(long)]
    pub shell: bool,
}

#[derive(Args)]
pub struct AliasExportArgs {
    /// Output file (stdout if not specified)
    #[arg(short = 'O', long)]
    pub output: Option<std::path::PathBuf>,
}

#[derive(clap::ValueEnum, Clone)]
pub enum ShellType {
    Zsh,
    Bash,
    Fish,
}

#[derive(clap::ValueEnum, Clone)]
pub enum ExportFormat {
    Json,
    Csv,
    Tsv,
    Plain,
}
