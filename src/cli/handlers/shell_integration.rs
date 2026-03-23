//! Shell integration handlers for zam CLI

use crate::cli::CliApp;
use crate::cli::args::*;
use crate::error::Result;

pub fn handle_shell(app: &CliApp, args: &ShellArgs) -> Result<()> {
    let shell_script = match args.shell {
        ShellType::Zsh => generate_zsh_integration(),
        ShellType::Bash => generate_bash_integration(),
        ShellType::Fish => generate_fish_integration(),
    };

    if let Some(output_file) = &args.output {
        std::fs::write(output_file, shell_script)?;
        if !app.quiet {
            println!("Shell integration written to {}", output_file.display());
        }
    } else {
        print!("{}", shell_script);
    }

    Ok(())
}

fn generate_zsh_integration() -> String {
    r#"# Zam Zsh Integration
# Add this to your ~/.zshrc

# One session per shell instance
export ZAM_SESSION_ID="zsh-$$-$(date +%s)"

# Log only successful commands
_zam_last_cmd=""
_zam_preexec() { _zam_last_cmd="$1"; }
_zam_precmd() {
    local rc=$?
    if [[ $rc -eq 0 && -n "$_zam_last_cmd" ]]; then
        zam log "$_zam_last_cmd" --session-id "$ZAM_SESSION_ID"
    fi
    _zam_last_cmd=""
}
_zam_zshexit() {
    zam end-session "$ZAM_SESSION_ID" 2>/dev/null
}
autoload -Uz add-zsh-hook
add-zsh-hook preexec _zam_preexec
add-zsh-hook precmd _zam_precmd
add-zsh-hook zshexit _zam_zshexit

# Interactive TUI history browser (Ctrl+R)
zam-widget() {
    local cmd="$(zam tui)"
    if [[ -n "$cmd" ]]; then
        BUFFER="$cmd"
        zle accept-line
    fi
    zle reset-prompt
}
zle -N zam-widget
bindkey '^R' zam-widget

# Load zam aliases into shell
eval "$(zam alias list --shell 2>/dev/null)"
"#
    .to_string()
}

fn generate_bash_integration() -> String {
    r#"# Zam Bash Integration
# Add this to your ~/.bashrc

# One session per shell instance
export ZAM_SESSION_ID="bash-$$-$(date +%s)"

# Function to log commands
log_command() {
    zam log "$1" --session-id "$ZAM_SESSION_ID"
}

# Close session on shell exit
trap 'zam end-session "$ZAM_SESSION_ID" 2>/dev/null' EXIT

# Hook to log commands after execution
PROMPT_COMMAND="log_command \"\$BASH_COMMAND\"; $PROMPT_COMMAND"

# Interactive history search with fzf (Ctrl+R)
bind -x '"\C-r": "READLINE_LINE=$(zam fzf | fzf --height 50% --reverse --tac 2>/dev/tty); READLINE_POINT=${#READLINE_LINE}"'

# Load zam aliases into shell
eval "$(zam alias list --shell 2>/dev/null)"
"#
    .to_string()
}

fn generate_fish_integration() -> String {
    r#"# Zam Fish Integration
# Add this to your ~/.config/fish/config.fish

# One session per shell instance
set -gx ZAM_SESSION_ID "fish-"(echo %self)"-"(date +%s)

# Close session on shell exit
function _zam_exit --on-event fish_exit
    zam end-session "$ZAM_SESSION_ID" 2>/dev/null
end

# Function to log commands
function zam_log_command --on-event fish_preexec
    zam log "$argv[1]" --session-id "$ZAM_SESSION_ID" &
end

# Interactive history search with fzf (Ctrl+R)
function zam_fzf_search
    set -l result (zam fzf | fzf --height 50% --reverse --tac 2>/dev/tty)
    if test -n "$result"
        commandline -r "$result"
    end
    commandline -f repaint
end

# Replace default Ctrl-R with fzf search
bind \cr zam_fzf_search

# Load zam aliases into shell
eval (zam alias list --shell 2>/dev/null)
"#
    .to_string()
}
