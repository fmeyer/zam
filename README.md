# Mortimer - Custom History Manager

Mortimer is a command-line utility designed for enhanced shell history management. It allows for logging, searching, and importing of shell commands with special handling for sensitive information. With Mortimer, you can easily redact sensitive data from your history, perform fuzzy searches, and import history from Zsh.

## Features

- **Redact Sensitive Information:** Automatically detects and redacts keys, passwords, tokens, and secrets from commands before logging.
- **Command Logging:** Manually log commands with `mortimer log <command>`.
- **Zsh History Import:** Import command history from Zsh's `.histfile`.
- **Fuzzy and Exact Search:** Use `fzf` for fuzzy searching your command history, with customizable key bindings.
- **Stores UTC timestame and directory:** Allow context filtering using the current directory to show commands

## Installation

1. Clone the Mortimer repository:

    ```bash
    git clone https://github.com/fmeyer/mortimer.git
    cd mortimer
    ```

2. Build and install the executable using Rust:

    ```bash
    cargo install --path .
    ```

## Usage

Mortimer provides a command-line interface with several subcommands. Here's how you can use each one:

- **Log a Command:**

  Log a command manually to the custom history log:

  ```bash
  mortimer log "echo mypassword123"  # Sensitive elements like passwords will be redacted
  ```

- **Import Zsh History:**

  Import history from the default Zsh history file:

  ```bash
  mortimer import
  ```

- **Search Command History:**

  ```bash
  mortimer search -pwd (optional) ! not implemented yet 😅
  ```

## Shell Integration

To integrate Mortimer with your shell, you need to define custom widgets for history management and replace the standard key bindings.

### Zsh Configuration

Add the following lines to your `.zshrc` to log commands and enhance history search using `fzf`:

```bash
# Custom history manager function
log_command() {
    mortimer log "$1"
}
autoload -Uz add-zsh-hook
add-zsh-hook preexec log_command

# Custom history search with Ctrl-R for fuzzy search
mortimer-history-widget() {
  BUFFER=$(mortimer fzf | fzf --height 50% 2>/dev/null)
  CURSOR=$#BUFFER
  zle reset-prompt
}

# Custom history search with Ctrl-E for exact match
mortimer-history-exact-widget() {
  BUFFER=$(mortimer fzf | fzf -e -i --height 50% 2>/dev/null)
  CURSOR=$#BUFFER
  zle reset-prompt
}

zle -N mortimer-history-widget
zle -N mortimer-history-exact-widget

# Replace default Ctrl-R with the custom widget
bindkey '^R' mortimer-history-widget
bindkey '^E' mortimer-history-exact-widget
```
