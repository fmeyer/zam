use chrono::{TimeZone, Utc};
use regex::Regex;
use std::collections::HashSet;
use std::env;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};

const HISTORY_FILE: &str = ".mhist";

// Define sensitive patterns to redact
const SENSITIVE_PATTERNS: &[&str] = &[
    r"(?i)\bpassword=[^\n]*",              // Matches "password=value"
    r"(?i)\btoken=[^\n]*",                 // Matches "token=value"
    r"(?i)\bsecret=[^\n]*",                // Matches "secret=value"
    r"(?i)\bsecret\-\w+=[^\n]*",           // Matches "secret-*=value"
    r"(?i)(://[a-z0-9._%+-]+:)[^@]*(@.*)", // Matches "username:password@domain"
];

fn redact_command(command: &str) -> String {
    let mut redacted_command = command.to_string();
    for pattern in SENSITIVE_PATTERNS {
        let re = Regex::new(pattern).expect("Failed to compile regex");
        redacted_command = re
            .replace_all(&redacted_command, |caps: &regex::Captures| {
                match caps.len() {
                    3 => format!("{}<redacted>{}", &caps[1], &caps[2]),
                    _ => format!(
                        "{}<redacted>",
                        &caps[0][..caps[0].find('=').unwrap_or(0) + 1]
                    ),
                }
            })
            .to_string();
    }
    redacted_command
}

fn log_command(command: &str, timestamp: Option<i64>) {
    // Redact sensitive parts of the command
    let redacted_command = redact_command(command);

    // Get the timestamp
    let timestamp_str = if let Some(ts) = timestamp {
        Utc.timestamp_opt(ts, 0)
            .unwrap() // Convert Unix timestamp to UTC DateTime
            .format("%Y-%m-%d %H:%M:%S")
            .to_string()
    } else {
        Utc::now().format("%Y-%m-%d %H:%M:%S").to_string()
    };

    // Get the current working directory
    let cwd = env::current_dir()
        .unwrap_or_else(|_| "<unknown>".into())
        .to_string_lossy()
        .to_string();

    // Combine the timestamp, directory, and command
    let log_entry = format!("{} | {} | {}\n", timestamp_str, cwd, redacted_command);

    // Append to the custom history log file
    let log_file_path = dirs::home_dir()
        .unwrap_or_else(|| "/tmp".into())
        .join(HISTORY_FILE);

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_file_path)
        .expect("Failed to open or create the log file");

    file.write_all(log_entry.as_bytes())
        .expect("Failed to write to the log file");
}

fn import_zsh_history() {
    // FIXME: take from splicit file
    let zsh_history_path = dirs::home_dir()
        .unwrap_or_else(|| "/tmp".into())
        .join(".histfile");

    // Open the Zsh history file
    let file = File::open(zsh_history_path).expect("Failed to open Zsh history file");
    let reader = BufReader::new(file);

    // Open the custom history log file for appending
    let log_file_path = dirs::home_dir()
        .unwrap_or_else(|| "/tmp".into())
        .join(HISTORY_FILE);

    // Ensure the file exists
    let _log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_file_path)
        .expect("Failed to open custom history log file");

    // ZSH history lines have the format: ": 1609786800:0;command"
    let re = Regex::new(r"^: (\d+):\d+;(.*)").expect("Failed to create regex");

    print!("importing: ");

    for line in reader.lines() {
        // ignore if we fail to read line from file (e.g. due to invalid UTF-8 encoding)
        let line = line.unwrap_or("".to_string());

        // Match each line using regex
        if let Some(caps) = re.captures(&line) {
            if let (Some(ts_str), Some(command_str)) = (caps.get(1), caps.get(2)) {
                if let Ok(timestamp) = ts_str.as_str().parse::<i64>() {
                    let redacted_command = redact_command(command_str.as_str().trim());
                    log_command(&redacted_command, Some(timestamp));
                }
            }
        }
        print!(".")
    }

    println!("\nZsh history imported successfully.");
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.is_empty() {
        eprintln!("Usage: mortimer log <command> | fzf <term> | import");
        std::process::exit(1);
    }

    // Handle "log", "fzf", and "import" commands
    match args[0].as_str() {
        "log" => {
            if args.len() < 2 {
                eprintln!("Usage: mortimer log <command>");
                std::process::exit(1);
            }
            log_command(&args[1..].join(" "), None);
        }
        "search" => {
            eprintln!("Not implemented")
        }
        "fzf" => {
            search_history_fzf();
        }
        "import" => {
            import_zsh_history();
        }
        _ => {
            eprintln!("Unknown command. Use 'log', 'search', 'fzf' or 'import'.");
            std::process::exit(1);
        }
    }
}

fn search_history_fzf() {
    let log_file_path = dirs::home_dir()
        .unwrap_or_else(|| "/tmp".into())
        .join(HISTORY_FILE);

    let mut seen_commands: HashSet<String> = HashSet::new();

    let file = File::open(log_file_path).expect("Failed to open the log file");
    let reader = BufReader::new(file);

    for line in reader.lines() {
        let line = line.expect("Failed to read line").trim().to_string();

        // Use a simple split to parse the date, directory, and command
        if let Some((_, rest)) = line.split_once(" | ") {
            if let Some((_, command)) = rest.split_once(" | ") {
                if seen_commands.insert(command.to_string()) {
                    println!("{}", command);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_command() {
        let test_cases = vec![
            ("password=123456", "password=<redacted>"),
            ("token=abcd1234", "token=<redacted>"),
            ("secret=verysecret", "secret=<redacted>"),
            ("secret-key=another secret", "secret-key=<redacted>"),
            (
                "parameter with secret-key=another secret inside",
                "parameter with secret-key=<redacted>",
            ),
        ];

        for (input, expected) in test_cases {
            assert_eq!(redact_command(input), expected);
        }
    }

    #[test]
    fn test_redact_connection_string() {
        let input = "postgresql://myuser:mypassword@mydbinstance.xyz123abc456.us-west-2.rds.amazonaws.com:5432/mydatabase";
        let expected = "postgresql://myuser:<redacted>@mydbinstance.xyz123abc456.us-west-2.rds.amazonaws.com:5432/mydatabase";
        assert_eq!(redact_command(input), expected);
    }
}
