# zam(1) - shell history manager

[![CI](https://github.com/fmeyer/zam/actions/workflows/ci.yml/badge.svg)](https://github.com/fmeyer/zam/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

## SYNOPSIS

    zam <command> [options]

## DESCRIPTION

**zam** is a shell history manager with automatic sensitive data redaction,
alias management, SQLite storage, and multi-host session tracking.

## INSTALL

    cargo install --path .

Or from crates.io:

    cargo install zam

## COMMANDS

    log <command>       Log a command to history
    search <term>       Search command history
    recent              Show recent commands
    frequent            Show most-used commands
    import <shell>      Import history from zsh, bash, or fish
    export              Export history (json, csv, tsv, plain)
    stats               Display usage statistics
    status              Show backend and configuration
    clear               Clear history (with confirmation)
    config              Manage configuration
    validate            Test redaction patterns
    shell <type>        Generate shell integration script
    fzf                 Output commands for fzf integration
    tui                 Interactive entity browser (TUI)
    merge <db-file>     Merge another database
    tokens              Manage stored redacted tokens
    hosts               List tracked hosts
    sessions            List shell sessions
    alias               Manage shell aliases

## OPTIONS

    -c, --config <path>     Configuration file path
    -v, --verbose           Verbose output
    -q, --quiet             Suppress non-error output
        --no-color          Disable colored output
        --use-file          Force file-based backend

## USAGE

    # Log and search
    zam log "git push origin main"
    zam search "git"
    zam search --regex "git (commit|push)"
    zam search --fuzzy "dckr"
    zam recent --count 10

    # Interactive TUI browser
    zam tui

    # Shell integration (add to shell rc file)
    eval "$(zam shell zsh)"

    # Alias management
    zam alias add ll "ls -la" "long listing"
    zam alias list
    zam alias export

    # Import existing history
    zam import zsh
    zam import bash

    # Merge from another machine
    zam merge ~/backup/history.db

    # Check status
    zam status

## CLAUDE CODE INTEGRATION

zam supports static sessions via `--session-id`, so all commands logged by
a long-running tool like Claude Code are grouped under one session.

Add a hook to `.claude/settings.json`:

```json
{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": "zam log \"$TOOL_INPUT\" --session-id \"claude-${SESSION_ID}\""
          }
        ]
      }
    ]
  }
}
```

Commands can then be reviewed per session:

    zam sessions --show-commands "claude-<session-id>"

Without `--session-id`, each `zam log` invocation creates a new session.
The flag reuses an existing session or creates one with the given ID on
first use.

## CONFIGURATION

Default config file: `~/.zam.json`

    zam config --init       Generate default config
    zam config --show       Print current config

Key settings: redaction patterns, search defaults, shell integration
exclusions, import paths.

## REDACTION

Automatically detects and redacts passwords, API keys, tokens, connection
strings, bearer tokens, SSH keys, AWS credentials, and GitHub tokens.

Custom patterns can be added via configuration.

    zam validate "pattern" --test "test string"

## STORAGE

Default location: `~/.local/zam/`

- Database backend (default): `zam.db` -- SQLite with sessions, hosts, tokens
- File backend (`--use-file`): `zam.log` -- structured log format

## BUILDING

    git clone https://github.com/fmeyer/zam.git
    cd zam
    cargo build --release
    cargo test

## LICENSE

MIT
