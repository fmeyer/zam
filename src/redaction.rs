//! Redaction engine for Mortimer
//!
//! This module provides sophisticated redaction capabilities for sensitive data
//! in shell commands, including passwords, tokens, API keys, and other secrets.

use crate::error::{Error, Result};
use regex::Regex;
use std::collections::HashMap;
use std::sync::{Mutex, Once};

/// Built-in redaction patterns for common sensitive data
pub const BUILTIN_PATTERNS: &[&str] = &[
    // Password patterns
    r"(?i)password\s*[=:]\s*[^\s]+",
    r"(?i)pwd\s*[=:]\s*[^\s]+",
    r"(?i)pass\s*[=:]\s*[^\s]+",
    r"(?i)passwd\s*[=:]\s*[^\s]+",
    // Token patterns
    r"(?i)token\s*[=:]\s*[^\s]+",
    r"(?i)auth_token\s*[=:]\s*[^\s]+",
    r"(?i)access_token\s*[=:]\s*[^\s]+",
    r"(?i)refresh_token\s*[=:]\s*[^\s]+",
    // API key patterns
    r"(?i)api_key\s*[=:]\s*[^\s]+",
    r"(?i)apikey\s*[=:]\s*[^\s]+",
    r"(?i)key\s*[=:]\s*[a-zA-Z0-9]{16,}",
    // Secret patterns
    r"(?i)secret\s*[=:]\s*[^\s]+",
    r"(?i)secret[-_]key\s*[=:]\s*[^\s]+",
    r"(?i)client_secret\s*[=:]\s*[^\s]+",
    // Connection string patterns
    r"(?i)(://[^:/@]+:)[^@]*(@)",
    r"(?i)(mongodb://[^:]+:)[^@]*(@)",
    r"(?i)(postgresql://[^:]+:)[^@]*(@)",
    r"(?i)(mysql://[^:]+:)[^@]*(@)",
    // Bearer token patterns
    r"(?i)bearer\s+[a-zA-Z0-9._-]+",
    r"(?i)authorization:\s*bearer\s+[a-zA-Z0-9._-]+",
    // SSH key patterns
    r"-----BEGIN [A-Z ]+-----[^-]*-----END [A-Z ]+-----",
    r"ssh-[a-z0-9]+ [A-Za-z0-9+/=]+",
    // Private key patterns
    r"(?i)private_key\s*[=:]\s*[^\s]+",
    r"(?i)priv_key\s*[=:]\s*[^\s]+",
    // Certificate patterns
    r"(?i)cert\s*[=:]\s*[^\s]+",
    r"(?i)certificate\s*[=:]\s*[^\s]+",
    // AWS credentials
    r"(?i)aws_access_key_id\s*[=:]\s*[^\s]+",
    r"(?i)aws_secret_access_key\s*[=:]\s*[^\s]+",
    r"(?i)aws_session_token\s*[=:]\s*[^\s]+",
    // GitHub tokens
    r"(?i)github_token\s*[=:]\s*[^\s]+",
    r"(?i)gh_token\s*[=:]\s*[^\s]+",
    r"ghp_[a-zA-Z0-9]{36}",
    r"gho_[a-zA-Z0-9]{36}",
    r"ghu_[a-zA-Z0-9]{36}",
    r"ghs_[a-zA-Z0-9]{36}",
    r"ghr_[a-zA-Z0-9]{36}",
    // Generic patterns for common formats
    //r"['\"][a-zA-Z0-9]{32,}['\"]",  // Long quoted strings
    r"[a-zA-Z0-9]{40,}", // Long alphanumeric strings (potential hashes/tokens)
];

/// Redaction engine for processing commands and removing sensitive data
#[derive(Debug, Clone)]
pub struct RedactionEngine {
    patterns: Vec<CompiledPattern>,
    exclude_patterns: Vec<CompiledPattern>,
    placeholder: String,
    min_length: usize,
    env_vars: Vec<String>,
    redact_env_vars: bool,
}

/// A compiled regex pattern with metadata
#[derive(Debug, Clone)]
struct CompiledPattern {
    regex: Regex,
    pattern: String,
    replacement_type: ReplacementType,
}

/// Type of replacement to perform
#[derive(Debug, Clone)]
enum ReplacementType {
    /// Replace entire match with placeholder
    Full,
    /// Replace only the sensitive part (for connection strings)
    Partial { keep_groups: Vec<usize> },
}

/// Statistics about redaction operations
#[derive(Debug, Clone, Default)]
pub struct RedactionStats {
    pub total_commands: usize,
    pub redacted_commands: usize,
    pub patterns_matched: HashMap<String, usize>,
    pub env_vars_redacted: usize,
}

static COMPILED_BUILTIN_PATTERNS: Once = Once::new();
static BUILTIN_PATTERNS_CACHE: Mutex<Option<Vec<CompiledPattern>>> = Mutex::new(None);

impl RedactionEngine {
    /// Create a new redaction engine with default patterns
    pub fn new() -> Result<Self> {
        Self::with_config(
            true,
            Vec::new(),
            Vec::new(),
            "<redacted>".to_string(),
            3,
            Vec::new(),
            false,
        )
    }

    /// Create a new redaction engine with custom configuration
    pub fn with_config(
        use_builtin: bool,
        custom_patterns: Vec<String>,
        exclude_patterns: Vec<String>,
        placeholder: String,
        min_length: usize,
        env_vars: Vec<String>,
        redact_env_vars: bool,
    ) -> Result<Self> {
        let mut patterns = Vec::new();

        // Add built-in patterns if requested
        if use_builtin {
            patterns.extend(Self::get_builtin_patterns()?);
        }

        // Add custom patterns
        for pattern in custom_patterns {
            patterns.push(CompiledPattern {
                regex: Regex::new(&pattern)?,
                pattern: pattern.clone(),
                replacement_type: ReplacementType::Full,
            });
        }

        // Compile exclude patterns
        let exclude_patterns: Result<Vec<_>> = exclude_patterns
            .into_iter()
            .map(|pattern| {
                Ok(CompiledPattern {
                    regex: Regex::new(&pattern)?,
                    pattern: pattern.clone(),
                    replacement_type: ReplacementType::Full,
                })
            })
            .collect();

        Ok(Self {
            patterns,
            exclude_patterns: exclude_patterns?,
            placeholder,
            min_length,
            env_vars,
            redact_env_vars,
        })
    }

    /// Get compiled built-in patterns (cached)
    fn get_builtin_patterns() -> Result<Vec<CompiledPattern>> {
        COMPILED_BUILTIN_PATTERNS.call_once(|| {
            let mut patterns = Vec::new();

            for pattern in BUILTIN_PATTERNS {
                let replacement_type = if pattern.contains("://") && pattern.contains("@") {
                    // Connection string pattern - keep prefix and suffix
                    ReplacementType::Partial {
                        keep_groups: vec![1, 2],
                    }
                } else {
                    ReplacementType::Full
                };

                if let Ok(regex) = Regex::new(pattern) {
                    patterns.push(CompiledPattern {
                        regex,
                        pattern: pattern.to_string(),
                        replacement_type,
                    });
                }
            }

            if let Ok(mut cache) = BUILTIN_PATTERNS_CACHE.lock() {
                *cache = Some(patterns);
            }
        });

        let cache = BUILTIN_PATTERNS_CACHE
            .lock()
            .map_err(|_| Error::custom("Failed to lock builtin patterns cache"))?;

        cache
            .as_ref()
            .ok_or_else(|| Error::custom("Failed to initialize builtin patterns"))
            .map(|patterns| patterns.clone())
    }

    /// Redact sensitive information from a command
    pub fn redact(&self, command: &str) -> Result<String> {
        let mut result = command.to_string();

        // First, redact environment variables if enabled
        if self.redact_env_vars {
            result = self.redact_env_variables(&result)?;
        }

        // Apply redaction patterns
        for pattern in &self.patterns {
            // Skip if this match should be excluded
            if self.should_exclude(&result, pattern) {
                continue;
            }

            result = self.apply_pattern(&result, pattern)?;
        }

        Ok(result)
    }

    /// Redact with statistics tracking
    pub fn redact_with_stats(&self, command: &str, stats: &mut RedactionStats) -> Result<String> {
        let _original = command.to_string();
        let mut result = command.to_string();
        let mut was_redacted = false;

        stats.total_commands += 1;

        // First, redact environment variables if enabled
        if self.redact_env_vars {
            let env_redacted = self.redact_env_variables(&result)?;
            if env_redacted != result {
                stats.env_vars_redacted += 1;
                was_redacted = true;
            }
            result = env_redacted;
        }

        // Apply redaction patterns
        for pattern in &self.patterns {
            // Skip if this match should be excluded
            if self.should_exclude(&result, pattern) {
                continue;
            }

            let before = result.clone();
            result = self.apply_pattern(&result, pattern)?;

            if result != before {
                was_redacted = true;
                *stats
                    .patterns_matched
                    .entry(pattern.pattern.clone())
                    .or_insert(0) += 1;
            }
        }

        if was_redacted {
            stats.redacted_commands += 1;
        }

        Ok(result)
    }

    /// Apply a single pattern to the command
    fn apply_pattern(&self, command: &str, pattern: &CompiledPattern) -> Result<String> {
        match &pattern.replacement_type {
            ReplacementType::Full => Ok(pattern
                .regex
                .replace_all(command, &self.placeholder)
                .to_string()),
            ReplacementType::Partial { keep_groups } => {
                let result = pattern
                    .regex
                    .replace_all(command, |caps: &regex::Captures| {
                        let mut replacement = String::new();
                        for &group_idx in keep_groups {
                            if let Some(group) = caps.get(group_idx) {
                                replacement.push_str(group.as_str());
                                if group_idx == keep_groups[0] {
                                    replacement.push_str(&self.placeholder);
                                }
                            }
                        }
                        replacement
                    });
                Ok(result.to_string())
            }
        }
    }

    /// Check if a match should be excluded from redaction
    fn should_exclude(&self, text: &str, pattern: &CompiledPattern) -> bool {
        // Check if any exclude pattern matches
        for exclude_pattern in &self.exclude_patterns {
            if exclude_pattern.regex.is_match(text) {
                return true;
            }
        }

        // Check minimum length requirement
        if let Some(mat) = pattern.regex.find(text) {
            if mat.as_str().len() < self.min_length {
                return true;
            }
        }

        false
    }

    /// Redact environment variables from the command
    fn redact_env_variables(&self, command: &str) -> Result<String> {
        let mut result = command.to_string();

        for env_var in &self.env_vars {
            // Pattern for environment variable usage: $VAR, ${VAR}, or VAR=value
            let patterns = vec![
                format!(r"\$\{{{}\}}", regex::escape(env_var)),
                format!(r"\${}", regex::escape(env_var)),
                format!(r"{}=[^\s]+", regex::escape(env_var)),
            ];

            for pattern in patterns {
                let regex = Regex::new(&pattern)?;
                if pattern.contains("=") {
                    // For VAR=value pattern, keep the variable name
                    result = regex
                        .replace_all(&result, &format!("{}={}", env_var, self.placeholder))
                        .to_string();
                } else {
                    // For $VAR patterns, replace entirely
                    result = regex.replace_all(&result, &self.placeholder).to_string();
                }
            }
        }

        Ok(result)
    }

    /// Add a custom redaction pattern
    pub fn add_pattern(&mut self, pattern: String) -> Result<()> {
        let compiled = CompiledPattern {
            regex: Regex::new(&pattern)?,
            pattern: pattern.clone(),
            replacement_type: ReplacementType::Full,
        };
        self.patterns.push(compiled);
        Ok(())
    }

    /// Add an exclude pattern
    pub fn add_exclude_pattern(&mut self, pattern: String) -> Result<()> {
        let compiled = CompiledPattern {
            regex: Regex::new(&pattern)?,
            pattern: pattern.clone(),
            replacement_type: ReplacementType::Full,
        };
        self.exclude_patterns.push(compiled);
        Ok(())
    }

    /// Set the redaction placeholder
    pub fn set_placeholder(&mut self, placeholder: String) {
        self.placeholder = placeholder;
    }

    /// Set minimum length for redaction
    pub fn set_min_length(&mut self, min_length: usize) {
        self.min_length = min_length;
    }

    /// Get current redaction statistics
    pub fn get_stats(&self) -> RedactionStats {
        RedactionStats::default()
    }

    /// Check if a command contains sensitive data (without redacting)
    pub fn contains_sensitive_data(&self, command: &str) -> bool {
        for pattern in &self.patterns {
            if pattern.regex.is_match(command) && !self.should_exclude(command, pattern) {
                return true;
            }
        }
        false
    }

    /// Get all pattern strings for debugging
    pub fn get_patterns(&self) -> Vec<String> {
        self.patterns.iter().map(|p| p.pattern.clone()).collect()
    }
}

impl Default for RedactionEngine {
    fn default() -> Self {
        Self::new().expect("Failed to create default redaction engine")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_redaction() {
        let engine = RedactionEngine::new().unwrap();

        let test_cases = vec![
            ("password=secret123", "password=<redacted>"),
            ("token=abc123def456", "token=<redacted>"),
            ("api_key=very_secret_key", "api_key=<redacted>"),
            ("echo hello world", "echo hello world"), // No sensitive data
        ];

        for (input, expected) in test_cases {
            let result = engine.redact(input).unwrap();
            assert_eq!(result, expected, "Failed for input: {}", input);
        }
    }

    #[test]
    fn test_connection_string_redaction() {
        let engine = RedactionEngine::new().unwrap();

        let input = "postgresql://user:password123@localhost:5432/db";
        let result = engine.redact(input).unwrap();
        assert!(result.contains("postgresql://user:<redacted>@localhost:5432/db"));
    }

    #[test]
    fn test_bearer_token_redaction() {
        let engine = RedactionEngine::new().unwrap();

        let input = "curl -H 'Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9'";
        let result = engine.redact(input).unwrap();
        assert!(result.contains("<redacted>"));
        assert!(!result.contains("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9"));
    }

    #[test]
    fn test_custom_patterns() {
        let engine = RedactionEngine::with_config(
            false, // Don't use built-in patterns
            vec![r"custom_secret=\w+".to_string()],
            vec![],
            "<HIDDEN>".to_string(),
            1,
            vec![],
            false,
        )
        .unwrap();

        let input = "custom_secret=my_secret_value";
        let result = engine.redact(input).unwrap();
        assert_eq!(result, "<HIDDEN>");
    }

    #[test]
    fn test_exclude_patterns() {
        let engine = RedactionEngine::with_config(
            true,
            vec![],
            vec![r"test_password=\w+".to_string()], // Exclude test passwords
            "<redacted>".to_string(),
            1,
            vec![],
            false,
        )
        .unwrap();

        let input1 = "password=real_secret";
        let input2 = "test_password=fake_secret";

        let result1 = engine.redact(input1).unwrap();
        let result2 = engine.redact(input2).unwrap();

        assert!(result1.contains("<redacted>"));
        assert_eq!(result2, input2); // Should not be redacted
    }

    #[test]
    fn test_environment_variable_redaction() {
        let engine = RedactionEngine::with_config(
            false,
            vec![],
            vec![],
            "<redacted>".to_string(),
            1,
            vec!["SECRET_KEY".to_string()],
            true,
        )
        .unwrap();

        let test_cases = vec![
            (
                "export SECRET_KEY=my_secret",
                "export SECRET_KEY=<redacted>",
            ),
            ("echo $SECRET_KEY", "echo <redacted>"),
            ("echo ${SECRET_KEY}", "echo <redacted>"),
        ];

        for (input, expected) in test_cases {
            let result = engine.redact(input).unwrap();
            assert_eq!(result, expected, "Failed for input: {}", input);
        }
    }

    #[test]
    fn test_minimum_length() {
        let engine = RedactionEngine::with_config(
            false,
            vec![r"key=\w+".to_string()],
            vec![],
            "<redacted>".to_string(),
            10, // Minimum length of 10
            vec![],
            false,
        )
        .unwrap();

        let input1 = "key=short"; // 9 characters total, should not be redacted
        let input2 = "key=very_long_key"; // 17 characters total, should be redacted

        let result1 = engine.redact(input1).unwrap();
        let result2 = engine.redact(input2).unwrap();

        assert_eq!(result1, input1);
        assert_eq!(result2, "<redacted>");
    }

    #[test]
    fn test_github_token_patterns() {
        let engine = RedactionEngine::new().unwrap();

        let test_cases = vec![
            "ghp_1234567890abcdef1234567890abcdef123456",
            "gho_1234567890abcdef1234567890abcdef123456",
            "ghu_1234567890abcdef1234567890abcdef123456",
            "ghs_1234567890abcdef1234567890abcdef123456",
            "ghr_1234567890abcdef1234567890abcdef123456",
        ];

        for token in test_cases {
            let input = format!("git push https://{}@github.com/user/repo.git", token);
            let result = engine.redact(&input).unwrap();
            assert!(!result.contains(token), "Token {} was not redacted", token);
        }
    }

    #[test]
    fn test_contains_sensitive_data() {
        let engine = RedactionEngine::new().unwrap();

        assert!(engine.contains_sensitive_data("password=secret"));
        assert!(engine.contains_sensitive_data("token=abc123"));
        assert!(!engine.contains_sensitive_data("echo hello world"));
    }

    #[test]
    fn test_redaction_stats() {
        let engine = RedactionEngine::new().unwrap();
        let mut stats = RedactionStats::default();

        let commands = vec![
            "password=secret1",
            "token=secret2",
            "echo hello",
            "api_key=secret3",
        ];

        for cmd in commands {
            engine.redact_with_stats(cmd, &mut stats).unwrap();
        }

        assert_eq!(stats.total_commands, 4);
        assert_eq!(stats.redacted_commands, 3);
        assert!(!stats.patterns_matched.is_empty());
    }
}
