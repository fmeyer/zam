//! Sort modes for command lists in the TUI
//!
//! Each mode supplies a prior over commands: recency, frecency, or the
//! next-command prediction. Without a filter, commands are ordered by the
//! prior alone. With a filter, the fuzzy match score dominates and the prior
//! scales it, so a clearly better match still wins.

use crate::fuzzy::{self, Usage};
use chrono::{DateTime, Utc};

/// Weight of the recency boost in `Recent` mode (boost ≤ 0.8).
const RECENCY_WEIGHT: f64 = 0.5;
/// Weight of the normalized prediction boost in `Next` mode (boost ≤ 1.0).
const NEXT_WEIGHT: f64 = 1.0;

/// How command lists are ordered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortMode {
    /// Most recently used first
    #[default]
    Recent,
    /// Frecency: run count weighted by recency
    Frequent,
    /// Most likely next command in this shell session
    Next,
}

impl SortMode {
    /// The mode after this one, cycling.
    #[must_use]
    pub fn cycle(self) -> Self {
        match self {
            SortMode::Recent => SortMode::Frequent,
            SortMode::Frequent => SortMode::Next,
            SortMode::Next => SortMode::Recent,
        }
    }

    /// Short label, also used as the stored preference value.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            SortMode::Recent => "recent",
            SortMode::Frequent => "frequent",
            SortMode::Next => "next",
        }
    }

    /// Parse a stored preference value, defaulting to `Recent`.
    #[must_use]
    pub fn from_label(label: &str) -> Self {
        match label {
            "frequent" => SortMode::Frequent,
            "next" => SortMode::Next,
            _ => SortMode::Recent,
        }
    }
}

/// What the ranking needs to know about one command.
#[derive(Debug, Clone, Copy)]
pub struct RankInput {
    pub usage: Usage,
    /// Prediction score (only used in `Next` mode)
    pub next_score: f64,
}

/// Order `inputs` for display, returning their indices best first.
///
/// `matches` is `None` when there is no filter; otherwise `matches[i]` is the
/// fuzzy score of input `i`, or `None` if it does not match (and is dropped).
/// Ties always go to the most recently used command.
#[must_use]
pub fn order(
    mode: SortMode,
    inputs: &[RankInput],
    matches: Option<&[Option<u32>]>,
    now: DateTime<Utc>,
) -> Vec<usize> {
    let mut indices: Vec<usize> = match matches {
        Some(m) => (0..inputs.len()).filter(|&i| m[i].is_some()).collect(),
        None => (0..inputs.len()).collect(),
    };
    // Recency first, so the stable sort below breaks ties by recency.
    indices.sort_by(|&a, &b| inputs[b].usage.last_used.cmp(&inputs[a].usage.last_used));
    if mode == SortMode::Recent && matches.is_none() {
        return indices;
    }

    let max_next = indices
        .iter()
        .map(|&i| inputs[i].next_score)
        .fold(0.0, f64::max);
    let boost = |i: usize| -> f64 {
        let input = &inputs[i];
        match mode {
            SortMode::Recent => {
                RECENCY_WEIGHT * fuzzy::frecency(1, input.usage.last_used, now).ln_1p()
            }
            SortMode::Frequent => fuzzy::frecency_boost(&input.usage, now),
            SortMode::Next if max_next > 0.0 => NEXT_WEIGHT * input.next_score / max_next,
            SortMode::Next => 0.0,
        }
    };
    let key = |i: usize| -> f64 {
        match matches {
            Some(m) => f64::from(m[i].unwrap_or(0)) * (1.0 + boost(i)),
            None => boost(i),
        }
    };

    let mut keyed: Vec<(usize, f64)> = indices.into_iter().map(|i| (i, key(i))).collect();
    keyed.sort_by(|a, b| b.1.total_cmp(&a.1));
    keyed.into_iter().map(|(i, _)| i).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn input(count: usize, age_hours: i64, next_score: f64) -> RankInput {
        RankInput {
            usage: Usage {
                count,
                last_used: Utc::now() - Duration::hours(age_hours),
                in_cwd: false,
            },
            next_score,
        }
    }

    #[test]
    fn cycles_through_all_modes() {
        let mut mode = SortMode::default();
        let mut seen = Vec::new();
        for _ in 0..3 {
            seen.push(mode.label());
            assert_eq!(SortMode::from_label(mode.label()), mode);
            mode = mode.cycle();
        }
        assert_eq!(seen, vec!["recent", "frequent", "next"]);
        assert_eq!(mode, SortMode::Recent);
        assert_eq!(SortMode::from_label("bogus"), SortMode::Recent);
    }

    #[test]
    fn unfiltered_order_follows_mode_prior() {
        let now = Utc::now();
        // 0: recent but rare, 1: old but frequent, 2: predicted next
        let inputs = [input(1, 0, 0.0), input(100, 200, 0.1), input(2, 48, 0.9)];
        assert_eq!(order(SortMode::Recent, &inputs, None, now), vec![0, 2, 1]);
        assert_eq!(order(SortMode::Frequent, &inputs, None, now)[0], 1);
        assert_eq!(order(SortMode::Next, &inputs, None, now), vec![2, 1, 0]);
    }

    #[test]
    fn filter_drops_non_matches_and_keeps_match_quality_dominant() {
        let now = Utc::now();
        let inputs = [input(1, 0, 0.0), input(1, 1000, 1.0), input(1, 1, 0.5)];
        let matches = [Some(200), None, Some(100)];
        for mode in [SortMode::Recent, SortMode::Frequent, SortMode::Next] {
            assert_eq!(order(mode, &inputs, Some(&matches), now), vec![0, 2]);
        }
    }

    #[test]
    fn prior_breaks_close_matches() {
        let now = Utc::now();
        let inputs = [input(1, 0, 0.0), input(1, 1000, 1.0)];
        let matches = [Some(100), Some(100)];
        assert_eq!(
            order(SortMode::Recent, &inputs, Some(&matches), now),
            vec![0, 1]
        );
        assert_eq!(
            order(SortMode::Next, &inputs, Some(&matches), now),
            vec![1, 0]
        );
    }

    #[test]
    fn ties_go_to_most_recent() {
        let now = Utc::now();
        let inputs = [input(3, 5, 0.0), input(3, 2, 0.0)];
        assert_eq!(order(SortMode::Next, &inputs, None, now), vec![1, 0]);
    }
}
