//! Fuzzy matching and ranking shared by the TUI and `zam search`
//!
//! Matching is delegated to `nucleo-matcher`, a port of fzf's v2 algorithm:
//! it finds the best-scoring alignment (not the leftmost one) and rewards
//! word boundaries, camelCase and consecutive characters. Queries use fzf
//! syntax: space-separated terms are ANDed, `'foo` is a substring, `^foo` a
//! prefix, `foo$` a suffix and `!foo` a negation. Case is smart: a term is
//! case-insensitive unless it contains an uppercase char.
//!
//! Ranking blends match quality with a built-in frecency weight so commands
//! you run often and recently float up among equally good matches.

use crate::database::MatchMode;
use chrono::{DateTime, Utc};
use nucleo_matcher::pattern::{AtomKind, Normalization, Pattern};
use nucleo_matcher::{Config, Utf32Str};

pub use nucleo_matcher::pattern::CaseMatching;

/// Weight of `ln(1 + frecency)` in the rank multiplier.
const FRECENCY_WEIGHT: f64 = 0.15;
/// Rank multiplier bonus for commands previously run in the current directory.
const SAME_DIR_BONUS: f64 = 0.25;

/// Reusable matcher for a single query.
///
/// Holds nucleo's scratch buffers, so keep one around instead of creating a
/// new matcher per candidate.
pub struct Matcher {
    inner: nucleo_matcher::Matcher,
    pattern: Pattern,
    query: String,
    mode: MatchMode,
    case: CaseMatching,
    buf: Vec<char>,
    indices: Vec<u32>,
}

impl Matcher {
    /// Create a matcher for `query` in the given mode.
    #[must_use]
    pub fn new(query: &str, mode: MatchMode, case: CaseMatching) -> Self {
        let mut config = Config::DEFAULT;
        // Users usually type the start of a command.
        config.prefer_prefix = true;
        Self {
            inner: nucleo_matcher::Matcher::new(config),
            pattern: build_pattern(query, mode, case),
            query: query.to_string(),
            mode,
            case,
            buf: Vec::new(),
            indices: Vec::new(),
        }
    }

    /// Change the query, reparsing only if it actually changed.
    pub fn set_query(&mut self, query: &str, mode: MatchMode) {
        if self.query != query || self.mode != mode {
            self.pattern = build_pattern(query, mode, self.case);
            self.query = query.to_string();
            self.mode = mode;
        }
    }

    /// Whether the query has no terms (every text matches with score 0).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pattern.atoms.is_empty()
    }

    /// Score `text` against the query; `None` if it does not match.
    #[must_use]
    pub fn score(&mut self, text: &str) -> Option<u32> {
        let haystack = Utf32Str::new(text, &mut self.buf);
        self.pattern.score(haystack, &mut self.inner)
    }

    /// Sorted, deduplicated char indices of matched characters in `text`;
    /// `None` if it does not match.
    #[must_use]
    pub fn indices(&mut self, text: &str) -> Option<Vec<usize>> {
        self.score_with_indices(text).map(|(_, indices)| indices)
    }

    /// Score and matched char indices in one pass; `None` if no match.
    #[must_use]
    pub fn score_with_indices(&mut self, text: &str) -> Option<(u32, Vec<usize>)> {
        let haystack = Utf32Str::new(text, &mut self.buf);
        self.indices.clear();
        let score = self
            .pattern
            .indices(haystack, &mut self.inner, &mut self.indices)?;
        let mut out: Vec<usize> = self.indices.iter().map(|&i| i as usize).collect();
        out.sort_unstable();
        out.dedup();
        Some((score, out))
    }
}

fn build_pattern(query: &str, mode: MatchMode, case: CaseMatching) -> Pattern {
    match mode {
        MatchMode::Fuzzy => Pattern::parse(query, case, Normalization::Smart),
        MatchMode::Substring => {
            Pattern::new(query, case, Normalization::Smart, AtomKind::Substring)
        }
    }
}

/// How a command has been used, for frecency ranking.
#[derive(Debug, Clone, Copy)]
pub struct Usage {
    /// Number of times the command was run
    pub count: usize,
    /// When it was last run
    pub last_used: DateTime<Utc>,
    /// Whether it was ever run in the current directory
    pub in_cwd: bool,
}

/// Zoxide-style frecency: run count weighted by how recently it was used.
#[must_use]
pub fn frecency(count: usize, last_used: DateTime<Utc>, now: DateTime<Utc>) -> f64 {
    let age = now.signed_duration_since(last_used);
    let recency = if age < chrono::Duration::hours(1) {
        4.0
    } else if age < chrono::Duration::days(1) {
        2.0
    } else if age < chrono::Duration::weeks(1) {
        1.0
    } else {
        0.25
    };
    count as f64 * recency
}

/// Frecency and same-directory part of the rank multiplier (0 = no boost).
#[must_use]
pub fn frecency_boost(usage: &Usage, now: DateTime<Utc>) -> f64 {
    let dir_bonus = if usage.in_cwd { SAME_DIR_BONUS } else { 0.0 };
    FRECENCY_WEIGHT * frecency(usage.count, usage.last_used, now).ln_1p() + dir_bonus
}

/// Final rank for a matched command: match score scaled by frecency and a
/// same-directory bonus. Higher is better.
///
/// The multiplier grows logarithmically, so frecency reorders comparable
/// matches but cannot lift a poor match far above a clearly better one.
#[must_use]
pub fn rank(score: u32, usage: &Usage, now: DateTime<Utc>) -> f64 {
    f64::from(score) * (1.0 + frecency_boost(usage, now))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fuzzy(query: &str) -> Matcher {
        Matcher::new(query, MatchMode::Fuzzy, CaseMatching::Smart)
    }

    #[test]
    fn matches_subsequence_with_char_indices() {
        let mut m = fuzzy("gco");
        assert!(m.score("git checkout").is_some());
        assert!(m.score("xyz").is_none());
        // Non-ASCII chars before the match: indices are chars, not bytes.
        assert_eq!(fuzzy("ls").indices("café ls"), Some(vec![5, 6]));
    }

    #[test]
    fn prefers_word_boundaries_over_leftmost() {
        // Leftmost alignment would pick the scattered "s" of "ls" and "t" of
        // "-t"; best alignment matches the start of "status".
        let mut m = fuzzy("st");
        assert_eq!(m.indices("ls -t && git status"), Some(vec![13, 14]));
        let mut m = fuzzy("gst");
        let boundary = m.score("git status").unwrap();
        let scattered = m.score("xgxsxtx").unwrap();
        assert!(boundary > scattered);
    }

    #[test]
    fn supports_fzf_syntax() {
        // Terms are ANDed in any order.
        assert!(fuzzy("push git").score("git push origin").is_some());
        assert!(fuzzy("^git").score("sudo git pull").is_none());
        assert!(fuzzy("main$").score("git checkout main").is_some());
        assert!(fuzzy("git !push").score("git push").is_none());
        assert!(fuzzy("'chk").score("git checkout").is_none());
    }

    #[test]
    fn smart_case_and_normalization() {
        assert!(fuzzy("gco").score("Git CheckOut").is_some());
        assert!(fuzzy("GCO").score("git checkout").is_none());
        assert!(fuzzy("cafe").score("echo café").is_some());
    }

    #[test]
    fn substring_mode_requires_contiguous_terms() {
        let mut m = Matcher::new("check", MatchMode::Substring, CaseMatching::Smart);
        assert_eq!(m.indices("git checkout"), Some(vec![4, 5, 6, 7, 8]));
        m.set_query("gco", MatchMode::Substring);
        assert!(m.score("git checkout").is_none());
        m.set_query("gco", MatchMode::Fuzzy);
        assert!(m.score("git checkout").is_some());
    }

    #[test]
    fn empty_query_matches_everything() {
        let mut m = fuzzy("  ");
        assert!(m.is_empty());
        assert_eq!(m.score("anything"), Some(0));
    }

    #[test]
    fn frecency_decays_with_age() {
        let now = Utc::now();
        let recent = frecency(3, now - chrono::Duration::minutes(5), now);
        let today = frecency(3, now - chrono::Duration::hours(5), now);
        let old = frecency(3, now - chrono::Duration::days(30), now);
        assert!(recent > today && today > old);
        assert!(frecency(10, now, now) > frecency(1, now, now));
    }

    #[test]
    fn rank_blends_score_and_usage() {
        let now = Utc::now();
        let rare = Usage {
            count: 1,
            last_used: now - chrono::Duration::days(60),
            in_cwd: false,
        };
        let hot = Usage {
            count: 50,
            last_used: now,
            in_cwd: false,
        };
        // Same match quality: usage decides.
        assert!(rank(100, &hot, now) > rank(100, &rare, now));
        // Same-directory bonus.
        let here = Usage {
            in_cwd: true,
            ..rare
        };
        assert!(rank(100, &here, now) > rank(100, &rare, now));
        // A much better match still wins over a hot but poor one.
        assert!(rank(200, &rare, now) > rank(80, &hot, now));
    }
}
