//! Shell integration handlers for Mortimer CLI

use crate::cli::args::*;
use crate::cli::CliApp;
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

fn generate_bash_integration() -> String {
    r#"# Mortimer Bash Integration
# Add this to your ~/.bashrc

# Function to log commands
log_command() {
    mortimer log "$1"
}

# Hook to log commands after execution
PROMPT_COMMAND="log_command \"\$BASH_COMMAND\"; $PROMPT_COMMAND"

# Custom history search with Ctrl-R
bind -x '"\C-r": "READLINE_LINE=$(mortimer fzf | fzf --height 50% --reverse --tac 2>/dev/null); READLINE_POINT=${#READLINE_LINE}"'
"#
    .to_string()
}

fn generate_fish_integration() -> String {
    r#"# Mortimer Fish Integration
# Add this to your ~/.config/fish/config.fish

# Function to log commands
function mortimer_log_command --on-event fish_preexec
    mortimer log "$argv[1]" &
end

# Custom history search with Ctrl-R
function mortimer_search
    set -l result (mortimer fzf | fzf --height 50% --reverse --tac 2>/dev/null)
    if test -n "$result"
        commandline -r "$result"
    end
    commandline -f repaint
end

# Bind Ctrl-R to custom search
bind \cr mortimer_search
"#
    .to_string()
}
