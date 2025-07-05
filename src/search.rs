//! Search engine for Mortimer
//!
//! This module provides advanced search capabilities for command history,
//! including fuzzy search, filtering, ranking, and result highlighting.

use crate::error::Result;
use crate::history::HistoryEntry;
use regex::Regex;
use std::collections::HashMap;

/// Search engine for history entries
#[derive(Debug, Clone)]
pub struct SearchEngine {
    /// Whether to enable fuzzy search by default
    pub fuzzy_search: bool,
    /// Whether to enable case-sensitive search by default
    pub case_sensitive: bool,
    /// Whether to include directory information in search results
    pub include_directory: bool,
    /// Whether to include timestamps in search results
    pub include_timestamps: bool,
    /// Maximum number of search results to return
    pub max_results: usize,
    /// Whether to highlight matches in search results
    pub highlight_matches: bool,
}

/// Search query with various filters and options
#[derive(Debug, Clone)]
pub struct SearchQuery {
    /// The search term
    pub term: String,
    /// Optional directory filter
    pub directory: Option<String>,
    /// Optional time range filter (start, end)
    pub time_range: Option<(chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>)>,
    /// Whether to use fuzzy matching
    pub fuzzy: bool,
    /// Whether to use case-sensitive matching
    pub case_sensitive: bool,
    /// Whether to use regex matching
    pub regex: bool,
    /// Whether to search only in redacted commands
    pub redacted_only: bool,
    /// Maximum number of results to return
    pub limit: Option<usize>,
}

/// Search result with metadata
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// The history entry
    pub entry: HistoryEntry,
    /// Relevance score (higher is better)
    pub score: f64,
    /// Highlighted command (if highlighting is enabled)
    pub highlighted: Option<String>,
    /// Match positions in the command
    pub matches: Vec<(usize, usize)>,
}

/// Search statistics
#[derive(Debug, Clone, Default)]
pub struct SearchStats {
    /// Total entries searched
    pub total_searched: usize,
    /// Number of matches found
    pub matches_found: usize,
    /// Time taken for search (in milliseconds)
    pub search_time_ms: u64,
    /// Number of results returned (after limiting)
    pub results_returned: usize,
}

impl SearchEngine {
    /// Create a new search engine with default settings
    pub fn new() -> Self {
        Self {
            fuzzy_search: true,
            case_sensitive: false,
            include_directory: true,
            include_timestamps: false,
            max_results: 1000,
            highlight_matches: true,
        }
    }

    /// Create a new search engine with custom settings
    pub fn with_config(
        fuzzy_search: bool,
        case_sensitive: bool,
        include_directory: bool,
        include_timestamps: bool,
        max_results: usize,
        highlight_matches: bool,
    ) -> Self {
        Self {
            fuzzy_search,
            case_sensitive,
            include_directory,
            include_timestamps,
            max_results,
            highlight_matches,
        }
    }

    /// Search through history entries with a simple query
    pub fn search(&self, entries: &[HistoryEntry], query: &str) -> Result<Vec<SearchResult>> {
        let search_query = SearchQuery {
            term: query.to_string(),
            directory: None,
            time_range: None,
            fuzzy: self.fuzzy_search,
            case_sensitive: self.case_sensitive,
            regex: false,
            redacted_only: false,
            limit: Some(self.max_results),
        };

        self.search_with_query(entries, &search_query)
    }

    /// Search through history entries with a detailed query
    pub fn search_with_query(
        &self,
        entries: &[HistoryEntry],
        query: &SearchQuery,
    ) -> Result<Vec<SearchResult>> {
        let start_time = std::time::Instant::now();
        let mut results = Vec::new();
        let mut stats = SearchStats::default();

        // Compile regex if needed
        let regex = if query.regex {
            Some(if query.case_sensitive {
                Regex::new(&query.term)?
            } else {
                Regex::new(&format!("(?i){}", query.term))?
            })
        } else {
            None
        };

        // Prepare search term
        let search_term = if query.case_sensitive {
            query.term.clone()
        } else {
            query.term.to_lowercase()
        };

        for entry in entries {
            stats.total_searched += 1;

            // Apply filters
            if !self.matches_filters(entry, query) {
                continue;
            }

            // Check for match
            let (is_match, matches, score) = if let Some(ref regex) = regex {
                self.regex_match(&entry.command, regex)?
            } else if query.fuzzy {
                self.fuzzy_match(&entry.command, &search_term, query.case_sensitive)
            } else {
                self.exact_match(&entry.command, &search_term, query.case_sensitive)
            };

            if is_match {
                stats.matches_found += 1;

                let highlighted = if self.highlight_matches && !matches.is_empty() {
                    Some(self.highlight_command(&entry.command, &matches))
                } else {
                    None
                };

                results.push(SearchResult {
                    entry: entry.clone(),
                    score,
                    highlighted,
                    matches,
                });
            }
        }

        // Sort by score (descending) and then by timestamp (descending)
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.entry.timestamp.cmp(&a.entry.timestamp))
        });

        // Apply limit
        if let Some(limit) = query.limit {
            results.truncate(limit);
        }

        stats.results_returned = results.len();
        stats.search_time_ms = start_time.elapsed().as_millis() as u64;

        Ok(results)
    }

    /// Search for commands that contain sensitive data
    pub fn search_redacted(&self, entries: &[HistoryEntry]) -> Result<Vec<SearchResult>> {
        let mut results = Vec::new();

        for entry in entries {
            if entry.redacted {
                results.push(SearchResult {
                    entry: entry.clone(),
                    score: 1.0,
                    highlighted: None,
                    matches: Vec::new(),
                });
            }
        }

        // Sort by timestamp (descending)
        results.sort_by(|a, b| b.entry.timestamp.cmp(&a.entry.timestamp));

        if results.len() > self.max_results {
            results.truncate(self.max_results);
        }

        Ok(results)
    }

    /// Search for commands from a specific directory
    pub fn search_by_directory(
        &self,
        entries: &[HistoryEntry],
        directory: &str,
    ) -> Result<Vec<SearchResult>> {
        let mut results = Vec::new();

        for entry in entries {
            if entry.directory.contains(directory) {
                results.push(SearchResult {
                    entry: entry.clone(),
                    score: 1.0,
                    highlighted: None,
                    matches: Vec::new(),
                });
            }
        }

        // Sort by timestamp (descending)
        results.sort_by(|a, b| b.entry.timestamp.cmp(&a.entry.timestamp));

        if results.len() > self.max_results {
            results.truncate(self.max_results);
        }

        Ok(results)
    }

    /// Get the most frequently used commands
    pub fn get_frequent_commands(&self, entries: &[HistoryEntry]) -> Result<Vec<(String, usize)>> {
        let mut command_counts = HashMap::new();

        for entry in entries {
            *command_counts.entry(entry.command.clone()).or_insert(0) += 1;
        }

        let mut sorted_commands: Vec<(String, usize)> = command_counts.into_iter().collect();
        sorted_commands.sort_by(|a, b| b.1.cmp(&a.1));

        if sorted_commands.len() > self.max_results {
            sorted_commands.truncate(self.max_results);
        }

        Ok(sorted_commands)
    }

    /// Get the most frequently used directories
    pub fn get_frequent_directories(
        &self,
        entries: &[HistoryEntry],
    ) -> Result<Vec<(String, usize)>> {
        let mut directory_counts = HashMap::new();

        for entry in entries {
            *directory_counts.entry(entry.directory.clone()).or_insert(0) += 1;
        }

        let mut sorted_directories: Vec<(String, usize)> = directory_counts.into_iter().collect();
        sorted_directories.sort_by(|a, b| b.1.cmp(&a.1));

        if sorted_directories.len() > self.max_results {
            sorted_directories.truncate(self.max_results);
        }

        Ok(sorted_directories)
    }

    /// Check if an entry matches the query filters
    fn matches_filters(&self, entry: &HistoryEntry, query: &SearchQuery) -> bool {
        // Directory filter
        if let Some(ref dir_filter) = query.directory {
            if !entry.directory.contains(dir_filter) {
                return false;
            }
        }

        // Time range filter
        if let Some((start, end)) = query.time_range {
            if entry.timestamp < start || entry.timestamp > end {
                return false;
            }
        }

        // Redacted filter
        if query.redacted_only && !entry.redacted {
            return false;
        }

        true
    }

    /// Perform exact string matching
    fn exact_match(
        &self,
        command: &str,
        search_term: &str,
        case_sensitive: bool,
    ) -> (bool, Vec<(usize, usize)>, f64) {
        let haystack = if case_sensitive {
            command
        } else {
            &command.to_lowercase()
        };

        let mut matches = Vec::new();
        let mut start = 0;
        let mut match_count = 0;

        while let Some(pos) = haystack[start..].find(search_term) {
            let actual_pos = start + pos;
            matches.push((actual_pos, actual_pos + search_term.len()));
            start = actual_pos + search_term.len();
            match_count += 1;
        }

        let is_match = !matches.is_empty();
        let score = if is_match {
            // Higher score for more matches and exact matches at the beginning
            let base_score = match_count as f64;
            let position_bonus = if matches[0].0 == 0 { 0.5 } else { 0.0 };
            let length_ratio = search_term.len() as f64 / command.len() as f64;
            base_score + position_bonus + length_ratio
        } else {
            0.0
        };

        (is_match, matches, score)
    }

    /// Perform fuzzy matching using a simple algorithm
    fn fuzzy_match(
        &self,
        command: &str,
        search_term: &str,
        case_sensitive: bool,
    ) -> (bool, Vec<(usize, usize)>, f64) {
        let haystack = if case_sensitive {
            command.to_string()
        } else {
            command.to_lowercase()
        };

        let needle = if case_sensitive {
            search_term.to_string()
        } else {
            search_term.to_lowercase()
        };

        // Simple fuzzy matching: check if all characters in search term appear in order
        let mut matches = Vec::new();
        let mut haystack_pos = 0;
        let mut needle_pos = 0;
        let mut match_start = None;

        let haystack_chars: Vec<char> = haystack.chars().collect();
        let needle_chars: Vec<char> = needle.chars().collect();

        while haystack_pos < haystack_chars.len() && needle_pos < needle_chars.len() {
            if haystack_chars[haystack_pos] == needle_chars[needle_pos] {
                if match_start.is_none() {
                    match_start = Some(haystack_pos);
                }
                needle_pos += 1;
                if needle_pos == needle_chars.len() {
                    // Found all characters
                    matches.push((match_start.unwrap(), haystack_pos + 1));
                    break;
                }
            }
            haystack_pos += 1;
        }

        let is_match = needle_pos == needle_chars.len();
        let score = if is_match {
            // Calculate score based on how close the match is to exact
            let match_length = if let Some(start) = match_start {
                haystack_pos - start + 1
            } else {
                haystack.len()
            };
            let exact_ratio = needle.len() as f64 / match_length as f64;
            exact_ratio * 0.8 // Fuzzy matches score lower than exact matches
        } else {
            0.0
        };

        (is_match, matches, score)
    }

    /// Perform regex matching
    fn regex_match(
        &self,
        command: &str,
        regex: &Regex,
    ) -> Result<(bool, Vec<(usize, usize)>, f64)> {
        let mut matches = Vec::new();

        for mat in regex.find_iter(command) {
            matches.push((mat.start(), mat.end()));
        }

        let is_match = !matches.is_empty();
        let score = if is_match {
            // Score based on number of matches and total matched length
            let total_matched_length: usize = matches.iter().map(|(s, e)| e - s).sum();
            let match_ratio = total_matched_length as f64 / command.len() as f64;
            matches.len() as f64 + match_ratio
        } else {
            0.0
        };

        Ok((is_match, matches, score))
    }

    /// Highlight matches in a command
    fn highlight_command(&self, command: &str, matches: &[(usize, usize)]) -> String {
        if matches.is_empty() {
            return command.to_string();
        }

        let mut result = String::new();
        let mut last_end = 0;

        for &(start, end) in matches {
            // Add text before match
            if start > last_end {
                result.push_str(&command[last_end..start]);
            }

            // Add highlighted match
            result.push_str("\x1b[1;33m"); // Bold yellow
            result.push_str(&command[start..end]);
            result.push_str("\x1b[0m"); // Reset

            last_end = end;
        }

        // Add remaining text
        if last_end < command.len() {
            result.push_str(&command[last_end..]);
        }

        result
    }
}

impl Default for SearchEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchQuery {
    /// Create a new simple search query
    pub fn new(term: String) -> Self {
        Self {
            term,
            directory: None,
            time_range: None,
            fuzzy: true,
            case_sensitive: false,
            regex: false,
            redacted_only: false,
            limit: None,
        }
    }

    /// Set directory filter
    pub fn with_directory(mut self, directory: String) -> Self {
        self.directory = Some(directory);
        self
    }

    /// Set time range filter
    pub fn with_time_range(
        mut self,
        start: chrono::DateTime<chrono::Utc>,
        end: chrono::DateTime<chrono::Utc>,
    ) -> Self {
        self.time_range = Some((start, end));
        self
    }

    /// Enable fuzzy matching
    pub fn fuzzy(mut self) -> Self {
        self.fuzzy = true;
        self
    }

    /// Enable case-sensitive matching
    pub fn case_sensitive(mut self) -> Self {
        self.case_sensitive = true;
        self
    }

    /// Enable regex matching
    pub fn regex(mut self) -> Self {
        self.regex = true;
        self
    }

    /// Search only redacted commands
    pub fn redacted_only(mut self) -> Self {
        self.redacted_only = true;
        self
    }

    /// Set result limit
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn create_test_entries() -> Vec<HistoryEntry> {
        vec![
            HistoryEntry {
                command: "echo hello world".to_string(),
                timestamp: Utc::now(),
                directory: "/home/user".to_string(),
                redacted: false,
                original: None,
            },
            HistoryEntry {
                command: "ls -la".to_string(),
                timestamp: Utc::now(),
                directory: "/home/user/documents".to_string(),
                redacted: false,
                original: None,
            },
            HistoryEntry {
                command: "password=<redacted>".to_string(),
                timestamp: Utc::now(),
                directory: "/home/user".to_string(),
                redacted: true,
                original: Some("password=secret123".to_string()),
            },
            HistoryEntry {
                command: "echo Hello World".to_string(),
                timestamp: Utc::now(),
                directory: "/tmp".to_string(),
                redacted: false,
                original: None,
            },
        ]
    }

    #[test]
    fn test_basic_search() {
        let engine = SearchEngine::new();
        let entries = create_test_entries();

        let results = engine.search(&entries, "echo").unwrap();
        assert_eq!(results.len(), 2);
        assert!(results[0].entry.command.contains("echo"));
        assert!(results[1].entry.command.contains("echo"));
    }

    #[test]
    fn test_case_sensitive_search() {
        let engine = SearchEngine::with_config(false, true, true, false, 1000, true);
        let entries = create_test_entries();

        let results = engine.search(&entries, "Hello").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].entry.command, "echo Hello World");
    }

    #[test]
    fn test_fuzzy_search() {
        let engine = SearchEngine::new();
        let entries = create_test_entries();

        let results = engine.search(&entries, "eh").unwrap();
        assert!(!results.is_empty());
        // Should match "echo" commands
    }

    #[test]
    fn test_directory_search() {
        let engine = SearchEngine::new();
        let entries = create_test_entries();

        let results = engine.search_by_directory(&entries, "documents").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].entry.command, "ls -la");
    }

    #[test]
    fn test_redacted_search() {
        let engine = SearchEngine::new();
        let entries = create_test_entries();

        let results = engine.search_redacted(&entries).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].entry.redacted);
    }

    #[test]
    fn test_query_with_filters() {
        let engine = SearchEngine::new();
        let entries = create_test_entries();

        let query = SearchQuery::new("echo".to_string())
            .with_directory("/home/user".to_string())
            .limit(1);

        let results = engine.search_with_query(&entries, &query).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].entry.directory.contains("/home/user"));
    }

    #[test]
    fn test_frequent_commands() {
        let engine = SearchEngine::new();
        let mut entries = create_test_entries();
        // Add duplicate commands
        entries.push(HistoryEntry {
            command: "echo hello world".to_string(),
            timestamp: Utc::now(),
            directory: "/home/user".to_string(),
            redacted: false,
            original: None,
        });

        let frequent = engine.get_frequent_commands(&entries).unwrap();
        assert!(!frequent.is_empty());
        assert_eq!(frequent[0].0, "echo hello world");
        assert_eq!(frequent[0].1, 2);
    }

    #[test]
    fn test_highlighting() {
        let engine = SearchEngine::new();
        let command = "echo hello world";
        let matches = vec![(0, 4), (5, 10)]; // "echo" and "hello"

        let highlighted = engine.highlight_command(command, &matches);
        assert!(highlighted.contains("\x1b[1;33m")); // Should contain color codes
        assert!(highlighted.contains("\x1b[0m")); // Should contain reset codes
    }

    #[test]
    fn test_regex_search() {
        let engine = SearchEngine::new();
        let entries = create_test_entries();

        let query = SearchQuery::new(r"echo.*world".to_string()).regex();
        let results = engine.search_with_query(&entries, &query).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_search_scoring() {
        let engine = SearchEngine::new();
        let entries = vec![
            HistoryEntry {
                command: "echo test".to_string(), // Should score higher (exact match at start)
                timestamp: Utc::now(),
                directory: "/home/user".to_string(),
                redacted: false,
                original: None,
            },
            HistoryEntry {
                command: "some echo command".to_string(), // Should score lower
                timestamp: Utc::now(),
                directory: "/home/user".to_string(),
                redacted: false,
                original: None,
            },
        ];

        let results = engine.search(&entries, "echo").unwrap();
        assert_eq!(results.len(), 2);
        // First result should have higher score
        assert!(results[0].score >= results[1].score);
    }
}
