# mortimer(1) - shell history manager

A command-line history manager with automatic sensitive data redaction,
SQLite storage, and multi-host session tracking.

## SYNOPSIS

    mortimer [OPTIONS] <COMMAND> [ARGS]

## INSTALL

    cargo install --path .

From source:

    git clone https://github.com/fmeyer/mortimer.git
    cd mortimer
    cargo build --release

## COMMANDS

    log <command>           Record a command in history
    search <term>           Search command history
    recent                  Show recent commands
    frequent                Show most used commands or directories
    stats                   Display usage statistics
    import <shell>          Import history from zsh, bash, or fish
    export                  Export history (json, csv, tsv, plain)
    merge <db-file>         Merge another database into this one
    tokens                  View stored redacted tokens
    hosts                   List tracked hosts
    sessions                List shell sessions
    alias                   Manage command aliases
    shell <shell>           Generate shell integration script
    config                  Manage configuration
    validate                Test redaction patterns
    fzf                     Output for fuzzy finder integration
    clear                   Clear history (with confirmation)
    status                  Show backend and configuration info

## OPTIONS

    --use-file              Use file backend instead of database
    --config <path>         Path to configuration file
    -v, --verbose           Enable verbose output

## USAGE

Log and search:

    $ mortimer log "echo hello world"
    $ mortimer search "git"
    $ mortimer search --fuzzy "git comm"
    $ mortimer search --regex "git (commit|push)"
    $ mortimer recent --count 10

Filter by directory or time:

    $ mortimer search --directory ~/projects "npm test"
    $ mortimer search --since 2024-01-01 --before 2024-12-31 "deploy"
    $ mortimer search --redacted-only

Manage tokens and sessions:

    $ mortimer tokens --session <id>
    $ mortimer tokens --directory ~/projects --show-values
    $ mortimer hosts --list
    $ mortimer sessions --host-id <id> --active

Aliases:

    $ mortimer alias add gs "git status"
    $ mortimer alias list
    $ mortimer alias remove gs

Import and export:

    $ mortimer import zsh
    $ mortimer export --format json --output backup.json
    $ mortimer merge ~/other-machine.db

## SHELL INTEGRATION

Add to your shell rc file to auto-log commands:

    # zsh
    eval "$(mortimer shell zsh)"

    # bash
    eval "$(mortimer shell bash)"

    # fish
    mortimer shell fish | source

## CONFIGURATION

Config file: `~/.mortimer.json`

    $ mortimer config --init      # generate default config
    $ mortimer config --show      # print current config

Key settings:

    history_file              Path to log file
    max_entries               Max entries to keep (0 = unlimited)
    enable_redaction          Toggle automatic redaction
    redaction.custom_patterns Custom regex patterns to redact
    search.fuzzy_search       Enable fuzzy matching by default
    shell_integration.auto_log    Auto-log commands
    shell_integration.exclude_commands    Commands to skip

## REDACTION

Mortimer automatically redacts sensitive data before storing commands:
passwords, API keys, tokens, connection strings, bearer tokens, SSH keys,
AWS credentials, and GitHub tokens.

Test a pattern:

    $ mortimer validate "custom_key=\\w+" --test "custom_key=secret123"

## STORAGE

Default storage: `~/.local/mortimer/`

- `mortimer.db` -- SQLite database (primary)
- `mortimer.log` -- append-only log (redundancy)

The database backend is the default. Use `--use-file` to opt into
file-only mode.

## BUILDING

    cargo build --release
    cargo test
    cargo clippy

## LICENSE

MIT
