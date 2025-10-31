# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Mortimer is a modern, fast, and secure command-line history manager written in Rust. It provides enhanced shell history management with automatic sensitive data redaction, multi-shell support, and dual storage backends (file-based and SQLite database).

Key features:
- Automatic sensitive data redaction (passwords, tokens, API keys)
- SQLite database backend with multi-host and session tracking
- Token storage and retrieval for redacted values
- Multi-shell support (Zsh, Bash, Fish)
- Advanced search with fuzzy matching and regex support
- Shell integration scripts

## Development Commands

### Building and Running
```bash
# Build the project
cargo build

# Build release version
cargo build --release

# Run the binary
cargo run -- <command>

# Install locally
cargo install --path .
```

### Testing
```bash
# Run all tests
cargo test

# Run tests with output visible
cargo test -- --nocapture

# Run a specific test module
cargo test redaction::tests

# Run tests for a specific file
cargo test --test history
```

### Code Quality
```bash
# Check for errors without building
cargo check

# Run clippy for linting
cargo clippy

# Format code
cargo fmt

# Check formatting without modifying
cargo fmt -- --check
```

## Architecture

### Dual Backend System

Mortimer supports two storage backends that share a common interface:

1. **File-based backend** (`history.rs`): Legacy `.mhist` file format
2. **Database backend** (`history_db.rs`): SQLite with advanced features

Backend selection logic (in `cli/mod.rs:114-138`):
- Explicitly via `--use-db` or `--use-file` flags
- Auto-detection: uses database if `.mhist.db` exists, otherwise file-based
- `HistoryBackend` enum unifies the two implementations

### Core Module Structure

**CLI Layer** (`cli/` directory):
- `args.rs`: Command-line argument definitions using clap
- `mod.rs`: Main CLI app structure with backend dispatch
- `handlers/`: Separate handler modules for each command category:
  - `basic.rs`: log, search, recent, frequent, validate
  - `config.rs`: configuration management
  - `database.rs`: migrate, merge, tokens, hosts, sessions
  - `import_export.rs`: import/export functionality
  - `shell_integration.rs`: shell integration script generation

**Core Library** (`src/` root):
- `history.rs`: File-based history manager (`HistoryManager`)
- `history_db.rs`: Database-backed history manager (`HistoryManagerDb`)
- `database.rs`: Low-level SQLite operations, schema, and queries
- `redaction.rs`: Sensitive data detection and redaction engine
- `search.rs`: Search functionality with fuzzy matching and regex
- `config.rs`: Configuration loading and management
- `error.rs`: Error types and Result alias

### Database Schema

SQLite schema (in `database.rs:100-180`):

**hosts**: Tracks different machines
- id, hostname, created_at

**sessions**: Shell session tracking
- id (UUID), host_id, started_at, ended_at

**commands**: Command history entries
- id, session_id, command, timestamp, directory, redacted, exit_code

**tokens**: Extracted redacted values for retrieval
- id, command_id, token_type, placeholder, original_value, created_at

Foreign key relationships: sessions → hosts, commands → sessions, tokens → commands

### Redaction Engine

The `RedactionEngine` (`redaction.rs`) provides:
- Built-in patterns for common secrets (passwords, API keys, tokens, connection strings, SSH keys, AWS credentials, GitHub tokens)
- Custom pattern support via regex
- Exclude patterns to prevent false positives
- Environment variable redaction
- Token extraction for later retrieval (database backend only)

Pattern types detected (see `BUILTIN_PATTERNS` in `redaction.rs:12-63`):
- Password/credential patterns
- API keys and tokens
- Connection strings (postgresql://, mysql://, mongodb://)
- Bearer tokens
- SSH/private keys
- AWS credentials
- GitHub tokens (ghp_, gho_, etc.)

### Configuration System

Configuration is JSON-based (`~/.mortimer.json`), with sections:
- `redaction`: Redaction behavior and custom patterns
- `import`: Shell history import settings
- `search`: Search behavior defaults
- `shell_integration`: Auto-logging and exclusion rules
- `logging`: Output preferences

Configuration hierarchy:
1. Explicit `--config` flag
2. Default location (`~/.mortimer.json`)
3. Built-in defaults (`Config::default()`)

### Search Engine

The `SearchEngine` (`search.rs`) supports:
- Fuzzy matching (configurable threshold)
- Regex patterns
- Case-sensitive/insensitive search
- Directory filtering
- Time range filtering
- Redacted-only filtering
- Result limiting
- Match highlighting

## Working with the Codebase

### Adding a New Command

1. Define args struct in `cli/args.rs`
2. Add variant to `Commands` enum in `cli/mod.rs`
3. Create handler function in appropriate `cli/handlers/` module
4. Add match arm in `CliApp::run()` to dispatch to handler
5. Handler can access both backends via pattern matching on `app.backend`

### Adding a New Redaction Pattern

Built-in patterns: Add to `BUILTIN_PATTERNS` in `redaction.rs`
Custom patterns: Users add via configuration

### Testing Approach

Tests are embedded in source files using `#[cfg(test)]` modules. Major test coverage exists in:
- `config.rs`: Configuration loading and defaults
- `redaction.rs`: Redaction pattern matching
- `search.rs`: Search functionality
- `history.rs`: File operations
- `database.rs`: Database operations
- `history_db.rs`: Database backend integration

Use `tempfile` crate for test isolation (see `dev-dependencies` in `Cargo.toml`).

### Database Migrations

If modifying schema:
1. Update `initialize_schema()` in `database.rs`
2. Existing databases auto-initialize missing tables/columns
3. Consider adding migration command if breaking changes are needed

### Backend Implementation Parity

When adding features, consider:
- Does it work with both backends?
- Should it be database-only? (sessions, tokens, hosts, merge)
- File-only features are deprecated in favor of database

Return appropriate errors for unsupported operations (e.g., token retrieval requires database backend).
