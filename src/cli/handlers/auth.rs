//! 1Password authentication handler for zam CLI

use crate::cli::args::*;
use crate::cli::{CliApp, HistoryBackend};
use crate::error::{Error, Result};
use std::process::Command;

/// Escape a value for safe use in shell single-quoted strings.
/// Wraps in single quotes, escaping embedded single quotes as '\''
fn shell_escape(value: &str) -> String {
    let escaped = value.replace('\'', "'\\''");
    format!("'{}'", escaped)
}

/// 1Password field from JSON output
#[derive(serde::Deserialize)]
struct OpField {
    label: String,
    value: Option<String>,
    section: Option<serde_json::Value>,
}

/// 1Password item from JSON output
#[derive(serde::Deserialize)]
struct OpItem {
    fields: Vec<OpField>,
}

/// Resolve the session ID from args or environment
fn resolve_session_id(args: &AuthArgs) -> Option<String> {
    args.session_id
        .clone()
        .or_else(|| std::env::var("ZAM_SESSION_ID").ok())
}

pub fn handle_auth(app: &mut CliApp, args: &AuthArgs) -> Result<()> {
    if args.clear {
        return handle_auth_clear(app, args);
    }

    if args.list {
        return handle_auth_list(app, args);
    }

    let Some(ref item) = args.item else {
        return Err(Error::custom(
            "Usage: zam auth <ITEM> [--session-id ID]\n       zam auth --list [--session-id ID]\n       zam auth --clear [--session-id ID]",
        ));
    };

    handle_auth_load(app, args, item)
}

fn handle_auth_load(app: &mut CliApp, args: &AuthArgs, item: &str) -> Result<()> {
    let output = match Command::new("op")
        .args(["item", "get", item, "--format", "json"])
        .output()
    {
        Ok(output) => output,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(Error::custom(
                "1Password CLI (op) not found. Install from https://1password.com/downloads/command-line/",
            ));
        }
        Err(e) => return Err(Error::custom(format!("Failed to run op: {}", e))),
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::custom(format!("op failed: {}", stderr.trim())));
    }

    let op_item: OpItem = serde_json::from_slice(&output.stdout)
        .map_err(|e| Error::custom(format!("Failed to parse op output: {}", e)))?;

    let session_id = resolve_session_id(args);
    let db = match &app.backend {
        HistoryBackend::Database(mgr) => Some(&mgr.db),
        HistoryBackend::File(_) => None,
    };

    let mut count = 0;
    let mut keys = Vec::new();
    for field in &op_item.fields {
        // Only export fields that have a section (skip built-in metadata)
        if field.section.is_none() {
            continue;
        }
        let Some(ref value) = field.value else {
            continue;
        };
        if value.is_empty() {
            continue;
        }

        let key = field
            .label
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect::<String>();

        if key.is_empty() {
            continue;
        }

        if args.export {
            println!("export {}={}", key, shell_escape(value));
        }
        keys.push(key.clone());
        count += 1;

        if let (Some(db), Some(sid)) = (db, &session_id) {
            let source = format!("1password:{}", item);
            let _ = db.store_session_secret(sid, &key, &source);
        }
    }

    if args.export {
        if !app.quiet {
            eprintln!("Loaded {} secrets from 1Password item '{}'", count, item);
        }
    } else {
        println!(
            "Found {} secrets in '{}': {}",
            count,
            item,
            keys.join(", ")
        );
        println!("\nUse zam-auth to load them into your shell:");
        println!("  zam-auth {}", item);
    }

    Ok(())
}

fn handle_auth_list(app: &mut CliApp, args: &AuthArgs) -> Result<()> {
    let db = match &app.backend {
        HistoryBackend::Database(mgr) => &mgr.db,
        HistoryBackend::File(_) => {
            return Err(Error::custom("auth --list requires database backend"));
        }
    };

    let session_id = resolve_session_id(args)
        .ok_or_else(|| Error::custom("--session-id required (or set ZAM_SESSION_ID)"))?;

    let secrets = db.get_session_secrets(&session_id)?;

    if secrets.is_empty() {
        eprintln!("No secrets loaded for session {}", session_id);
    } else {
        eprintln!("Secrets loaded in session {}:", session_id);
        for s in &secrets {
            eprintln!("  {} (from {})", s.key_name, s.source);
        }
    }

    Ok(())
}

fn handle_auth_clear(app: &mut CliApp, args: &AuthArgs) -> Result<()> {
    let db = match &app.backend {
        HistoryBackend::Database(mgr) => &mgr.db,
        HistoryBackend::File(_) => {
            return Err(Error::custom("auth --clear requires database backend"));
        }
    };

    let session_id = resolve_session_id(args)
        .ok_or_else(|| Error::custom("--session-id required (or set ZAM_SESSION_ID)"))?;

    let keys = db.clear_session_secrets(&session_id)?;

    for key in &keys {
        println!("unset {}", key);
    }

    if !app.quiet {
        eprintln!("Cleared {} secrets from session", keys.len());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_escape_simple() {
        assert_eq!(shell_escape("hello"), "'hello'");
    }

    #[test]
    fn test_shell_escape_single_quotes() {
        assert_eq!(shell_escape("it's"), "'it'\\''s'");
    }

    #[test]
    fn test_shell_escape_spaces_and_special() {
        assert_eq!(shell_escape("foo bar $HOME"), "'foo bar $HOME'");
    }

    #[test]
    fn test_op_json_parsing() {
        let json = r#"{
            "fields": [
                {"label": "username", "value": "admin", "section": {"id": "kv"}},
                {"label": "password", "value": "s3cret", "section": {"id": "kv"}},
                {"label": "notesPlain", "value": "some note", "section": null},
                {"label": "empty", "value": "", "section": {"id": "kv"}},
                {"label": "no_value", "value": null, "section": {"id": "kv"}}
            ]
        }"#;

        let item: OpItem = serde_json::from_str(json).unwrap();
        let exported: Vec<_> = item
            .fields
            .iter()
            .filter(|f| f.section.is_some())
            .filter(|f| f.value.as_ref().is_some_and(|v| !v.is_empty()))
            .map(|f| f.label.as_str())
            .collect();

        assert_eq!(exported, vec!["username", "password"]);
    }
}
