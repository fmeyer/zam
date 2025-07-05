# Mortimer - Enhanced Shell History Manager

[![Rust](https://github.com/fmeyer/mortimer/actions/workflows/rust.yml/badge.svg)](https://github.com/fmeyer/mortimer/actions/workflows/rust.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

Mortimer is a modern, fast, and secure command-line history manager designed for power users who want enhanced shell history management with automatic sensitive data redaction. It provides advanced search capabilities, multi-shell support, and comprehensive configuration options.

## ✨ Features

- **🔒 Automatic Sensitive Data Redaction**: Intelligently detects and redacts passwords, tokens, API keys, and other sensitive information
- **🔍 Advanced Search**: Fuzzy search, regex support, filtering by directory and time range
- **📚 Multi-Shell Support**: Import from Zsh, Bash, and Fish history files
- **⚙️ Highly Configurable**: JSON-based configuration with extensive customization options
- **🚀 Fast and Efficient**: Written in Rust for optimal performance
- **🎯 Smart Deduplication**: Automatic duplicate command filtering
- **📊 Comprehensive Statistics**: Detailed analytics about your command usage
- **🔗 Shell Integration**: Seamless integration with your favorite shell

## 🚀 Installation

### From Source

```bash
git clone https://github.com/fmeyer/mortimer.git
cd mortimer
cargo install --path .
```

### Using Cargo

```bash
cargo install mortimer
```

## 📖 Quick Start

### Basic Usage

```bash
# Log a command manually
mortimer log "echo hello world"

# Search your history
mortimer search "git"

# Import your existing shell history
mortimer import zsh

# Search with fuzzy matching
mortimer search --fuzzy "git comm"

# Search with regex
mortimer search --regex "git (commit|push)"

# Show recent commands
mortimer recent --count 10

# Show statistics
mortimer stats
```

### Shell Integration

To automatically log all commands and enable enhanced history search:

#### Zsh

```bash
# Generate and add to your .zshrc
mortimer shell zsh >> ~/.zshrc
```

#### Bash

```bash
# Generate and add to your .bashrc
mortimer shell bash >> ~/.bashrc
```

#### Fish

```bash
# Generate and add to your config.fish
mortimer shell fish >> ~/.config/fish/config.fish
```

## 📋 Commands

### Core Commands

- `log <command>` - Log a command to history
- `search <term>` - Search command history
- `import <shell>` - Import history from shell files
- `recent` - Show recent commands
- `stats` - Display usage statistics
- `clear` - Clear history (with confirmation)

### Advanced Commands

- `export` - Export history in various formats (JSON, CSV, TSV, plain text)
- `frequent` - Show most frequently used commands or directories
- `fzf` - Output commands for fuzzy finder integration
- `config` - Manage configuration
- `validate` - Test redaction patterns

### Search Options

```bash
# Basic search
mortimer search "docker"

# Directory-filtered search
mortimer search --directory "/home/user/projects" "npm"

# Time-based search
mortimer search --since "2024-01-01" --before "2024-12-31" "deploy"

# Exact matching (disable fuzzy search)
mortimer search --exact "git commit"

# Case-sensitive search
mortimer search --case-sensitive "Docker"

# Regex search
mortimer search --regex "git (push|pull) origin"

# Search only redacted commands
mortimer search --redacted-only
```

## ⚙️ Configuration

Mortimer uses a JSON configuration file located at `~/.mortimer.json`. Generate a default configuration:

```bash
mortimer config --init
```

### Configuration Options

```json
{
  "history_file": "/home/user/.mhist",
  "max_entries": 100000,
  "enable_redaction": true,
  "redaction": {
    "placeholder": "<redacted>",
    "use_builtin_patterns": true,
    "custom_patterns": [
      "my_secret_pattern=\\w+"
    ],
    "exclude_patterns": [
      "test_password=\\w+"
    ],
    "redact_env_vars": true,
    "min_redaction_length": 3
  },
  "import": {
    "shell_history_paths": {
      "zsh": "/home/user/.histfile",
      "bash": "/home/user/.bash_history",
      "fish": "/home/user/.local/share/fish/fish_history"
    },
    "auto_detect": true,
    "deduplicate": true,
    "preserve_timestamps": true,
    "max_age_days": 0
  },
  "search": {
    "fuzzy_search": true,
    "case_sensitive": false,
    "include_directory": true,
    "include_timestamps": false,
    "max_results": 1000,
    "highlight_matches": true
  },
  "shell_integration": {
    "auto_log": true,
    "exclude_commands": ["ls", "cd", "pwd", "clear", "history"],
    "log_space_prefixed": false,
    "log_duplicates": false,
    "min_command_length": 1
  }
}
```

## 🔒 Security Features

### Built-in Redaction Patterns

Mortimer automatically detects and redacts:

- **Passwords**: `password=secret`, `pwd=secret`
- **Tokens**: `token=abc123`, `auth_token=xyz789`
- **API Keys**: `api_key=key123`, `apikey=key456`
- **Secrets**: `secret=hidden`, `client_secret=private`
- **Connection Strings**: `postgresql://user:pass@host/db`
- **Bearer Tokens**: `Authorization: Bearer token123`
- **SSH Keys**: Private key blocks and SSH public keys
- **AWS Credentials**: AWS access keys and session tokens
- **GitHub Tokens**: GitHub personal access tokens (ghp_, gho_, etc.)

### Custom Redaction

Add your own patterns:

```bash
# Test a pattern
mortimer validate "custom_key=\\w+" --test "custom_key=secret123"

# Add to configuration
mortimer config --set redaction.custom_patterns='["custom_key=\\w+"]'
```

## 📊 Statistics and Analytics

```bash
# Basic statistics
mortimer stats

# Detailed statistics with redaction info
mortimer stats --detailed --redaction

# Directory usage statistics
mortimer stats --directories

# Most frequent commands
mortimer frequent --count 20

# Most frequent directories
mortimer frequent --directories --count 10
```

## 🔍 Advanced Search Examples

```bash
# Find all git commands from last week
mortimer search --since "2024-01-15" "git"

# Find Docker commands in a specific directory
mortimer search --directory "/home/user/projects" "docker"

# Find commands that were redacted (contained sensitive data)
mortimer search --redacted-only

# Complex regex search
mortimer search --regex "curl.*(-H|--header).*Authorization"

# Case-sensitive search for specific commands
mortimer search --case-sensitive "Docker" --exact
```

## 🔧 Integration Examples

### With fzf (Fuzzy Finder)

```bash
# Interactive command selection
mortimer fzf | fzf --height 50% --reverse

# Bind to Ctrl+R in your shell (automatically done with shell integration)
# Zsh: bindkey '^R' mortimer-history-widget
# Bash: bind -x '"\C-r": mortimer_search'
```

### Export and Backup

```bash
# Export to JSON
mortimer export --format json --output history_backup.json

# Export specific directory commands
mortimer export --directory "/home/user/projects" --format csv

# Export recent commands (last 30 days)
mortimer export --days 30 --format plain
```

## 🏗️ Development

### Building from Source

```bash
git clone https://github.com/fmeyer/mortimer.git
cd mortimer
cargo build --release
```

### Running Tests

```bash
# Run all tests
cargo test

# Run with coverage
cargo test --all-features

# Run specific test module
cargo test history::tests
```

### Contributing

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## 📝 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request. For major changes, please open an issue first to discuss what you would like to change.

## 📞 Support

- 📧 Create an issue on GitHub
- 💬 Join our discussions
- 📖 Check the documentation

## 🙏 Acknowledgments

- Built with [Rust](https://www.rust-lang.org/)
- CLI powered by [clap](https://github.com/clap-rs/clap)
- Regex processing by [regex](https://github.com/rust-lang/regex)
- Date/time handling by [chrono](https://github.com/chronotope/chrono)

---

**Made with ❤️ by the Mortimer team**