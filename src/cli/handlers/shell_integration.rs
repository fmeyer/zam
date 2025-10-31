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

# Interactive history search with fzf (Ctrl+R)
mortimer-fzf-widget() {
    BUFFER=$(mortimer fzf | fzf --height 50% --reverse --tac 2>/dev/tty)
    CURSOR=$#BUFFER
    zle reset-prompt
}
zle -N mortimer-fzf-widget

# Replace default Ctrl-R with fzf search
bindkey '^R' mortimer-fzf-widget
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

# Interactive history search with fzf (Ctrl+R)
bind -x '"\C-r": "READLINE_LINE=$(mortimer fzf | fzf --height 50% --reverse --tac 2>/dev/tty); READLINE_POINT=${#READLINE_LINE}"'
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

# Interactive history search with fzf (Ctrl+R)
function mortimer_fzf_search
    set -l result (mortimer fzf | fzf --height 50% --reverse --tac 2>/dev/tty)
    if test -n "$result"
        commandline -r "$result"
    end
    commandline -f repaint
end

# Replace default Ctrl-R with fzf search
bind \cr mortimer_fzf_search
"#
    .to_string()
}
