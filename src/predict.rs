//! Next-command prediction from shell history
//!
//! An interpolated n-gram model over each shell session's command sequence.
//! A candidate's score is a weighted sum of its probability under several
//! conditional distributions:
//!
//! - trigram: given the previous two commands in the session
//! - bigram: given the previous command
//! - template: given the previous command's first two words (`git push`),
//!   which backs off the sparse exact-command bigram
//! - failure: given the previous command's template, when it failed
//! - cwd: given the current directory
//! - global: overall frequency
//!
//! [`evaluate`] replays history in time order to measure hit@k and MRR, so
//! weight changes can be compared on real data.

use crate::database::CommandEntry;
use std::collections::{BTreeSet, HashMap};
use std::fmt;
use std::str::FromStr;

/// Directory marker for commands imported from shell history files, which
/// carry no session ordering or directory.
const IMPORTED_DIR: &str = "<imported>";
/// Candidates taken from the top of each distribution before exact scoring.
const CANDIDATES_PER_SOURCE: usize = 50;
/// Cutoffs reported by [`evaluate`].
const HIT_CUTOFFS: [usize; 3] = [1, 5, 10];

/// Blend weights for each distribution.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Weights {
    pub trigram: f64,
    pub bigram: f64,
    pub template: f64,
    pub failure: f64,
    pub cwd: f64,
    pub global: f64,
}

impl Default for Weights {
    fn default() -> Self {
        // Chosen with `zam predict --eval`: trigram and failure added nothing
        // measurable on real history, so they are off by default.
        Self {
            trigram: 0.0,
            bigram: 2.0,
            template: 0.5,
            failure: 0.0,
            cwd: 1.0,
            global: 0.1,
        }
    }
}

impl Weights {
    const ZERO: Self = Self {
        trigram: 0.0,
        bigram: 0.0,
        template: 0.0,
        failure: 0.0,
        cwd: 0.0,
        global: 0.0,
    };

    /// Single-distribution baselines plus the default blend, for comparison.
    #[must_use]
    pub fn presets() -> Vec<(&'static str, Weights)> {
        vec![
            (
                "global",
                Self {
                    global: 1.0,
                    ..Self::ZERO
                },
            ),
            (
                "cwd",
                Self {
                    cwd: 1.0,
                    ..Self::ZERO
                },
            ),
            (
                "bigram",
                Self {
                    bigram: 1.0,
                    ..Self::ZERO
                },
            ),
            ("default blend", Self::default()),
        ]
    }
}

impl FromStr for Weights {
    type Err = String;

    /// Parse `trigram,bigram,template,failure,cwd,global`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let values = s
            .split(',')
            .map(|v| v.trim().parse::<f64>())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("invalid weight: {e}"))?;
        let [trigram, bigram, template, failure, cwd, global] = values[..] else {
            return Err(format!(
                "expected 6 weights (trigram,bigram,template,failure,cwd,global), got {}",
                values.len()
            ));
        };
        Ok(Self {
            trigram,
            bigram,
            template,
            failure,
            cwd,
            global,
        })
    }
}

impl fmt::Display for Weights {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{},{},{},{},{},{}",
            self.trigram, self.bigram, self.template, self.failure, self.cwd, self.global
        )
    }
}

/// What is known when predicting the next command in a session.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Context {
    pub prev: Option<String>,
    pub prev2: Option<String>,
    pub prev_failed: bool,
    pub directory: String,
}

/// Frequency distribution that can list its most common entries cheaply.
#[derive(Debug, Default)]
struct Dist {
    counts: HashMap<String, u64>,
    ranked: BTreeSet<(u64, String)>,
    total: u64,
}

impl Dist {
    fn add(&mut self, command: &str) {
        let count = self.counts.entry(command.to_string()).or_insert(0);
        if *count > 0 {
            self.ranked.remove(&(*count, command.to_string()));
        }
        *count += 1;
        self.ranked.insert((*count, command.to_string()));
        self.total += 1;
    }

    fn prob(&self, command: &str) -> f64 {
        match self.counts.get(command) {
            Some(&n) => n as f64 / self.total as f64,
            None => 0.0,
        }
    }

    fn top(&self, m: usize) -> impl Iterator<Item = &str> {
        self.ranked.iter().rev().take(m).map(|(_, c)| c.as_str())
    }
}

/// First two words of a command, used to back off from exact commands.
fn template(command: &str) -> String {
    command
        .split_whitespace()
        .take(2)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Online next-command predictor.
#[derive(Debug, Default)]
pub struct Predictor {
    weights: Weights,
    trigram: HashMap<(String, String), Dist>,
    bigram: HashMap<String, Dist>,
    template: HashMap<String, Dist>,
    failure: HashMap<String, Dist>,
    cwd: HashMap<String, Dist>,
    global: Dist,
    last_seen: HashMap<String, usize>,
    clock: usize,
}

impl Predictor {
    #[must_use]
    pub fn new(weights: Weights) -> Self {
        Self {
            weights,
            ..Self::default()
        }
    }

    /// Learn that `command` was run in `ctx`.
    pub fn observe(&mut self, ctx: &Context, command: &str) {
        if let Some(prev) = &ctx.prev {
            self.bigram.entry(prev.clone()).or_default().add(command);
            let prev_template = template(prev);
            if ctx.prev_failed {
                self.failure
                    .entry(prev_template.clone())
                    .or_default()
                    .add(command);
            }
            self.template.entry(prev_template).or_default().add(command);
            if let Some(prev2) = &ctx.prev2 {
                self.trigram
                    .entry((prev2.clone(), prev.clone()))
                    .or_default()
                    .add(command);
            }
        }
        self.cwd
            .entry(ctx.directory.clone())
            .or_default()
            .add(command);
        self.global.add(command);
        self.clock += 1;
        self.last_seen.insert(command.to_string(), self.clock);
    }

    /// Whether `command` has been observed before.
    #[must_use]
    pub fn has_seen(&self, command: &str) -> bool {
        self.global.counts.contains_key(command)
    }

    /// Scorer for arbitrary commands in `ctx`, for ranking an existing list.
    #[must_use]
    pub fn scorer(&self, ctx: &Context) -> Scorer<'_> {
        let w = &self.weights;
        let prev_template = ctx.prev.as_deref().map(template);
        let trigram = match (&ctx.prev2, &ctx.prev) {
            (Some(p2), Some(p)) => self.trigram.get(&(p2.clone(), p.clone())),
            _ => None,
        };
        let failure = if ctx.prev_failed {
            prev_template.as_ref().and_then(|t| self.failure.get(t))
        } else {
            None
        };
        let sources: [(f64, Option<&Dist>); 6] = [
            (w.trigram, trigram),
            (w.bigram, ctx.prev.as_ref().and_then(|p| self.bigram.get(p))),
            (
                w.template,
                prev_template.as_ref().and_then(|t| self.template.get(t)),
            ),
            (w.failure, failure),
            (w.cwd, self.cwd.get(&ctx.directory)),
            (w.global, Some(&self.global)),
        ];

        Scorer {
            active: sources
                .into_iter()
                .filter_map(|(weight, dist)| Some((weight, dist?)))
                .filter(|(weight, _)| *weight > 0.0)
                .collect(),
        }
    }

    /// Top `k` predicted commands for `ctx`, best first.
    #[must_use]
    pub fn predict(&self, ctx: &Context, k: usize) -> Vec<(String, f64)> {
        let scorer = self.scorer(ctx);
        let candidates: BTreeSet<&str> = scorer
            .active
            .iter()
            .flat_map(|(_, dist)| dist.top(CANDIDATES_PER_SOURCE))
            .collect();

        let mut scored: Vec<(&str, f64)> = candidates
            .into_iter()
            .map(|c| (c, scorer.score(c)))
            .collect();
        // Best score first; ties go to the most recently used command.
        scored.sort_by(|a, b| {
            b.1.total_cmp(&a.1).then_with(|| {
                let seen = |c: &str| self.last_seen.get(c).copied().unwrap_or(0);
                seen(b.0).cmp(&seen(a.0))
            })
        });
        scored
            .into_iter()
            .take(k)
            .map(|(c, s)| (c.to_string(), s))
            .collect()
    }
}

/// Scores commands against one context (see [`Predictor::scorer`]).
pub struct Scorer<'a> {
    active: Vec<(f64, &'a Dist)>,
}

impl Scorer<'_> {
    /// Blended probability that `command` runs next; 0 if never seen.
    #[must_use]
    pub fn score(&self, command: &str) -> f64 {
        self.active
            .iter()
            .map(|(w, dist)| w * dist.prob(command))
            .sum()
    }
}

/// Build a predictor from prepared history and the context for the next
/// command in `session_id` run from `directory`.
#[must_use]
pub fn train(
    history: &[CommandEntry],
    weights: Weights,
    session_id: &str,
    directory: &str,
) -> (Predictor, Context) {
    let mut predictor = Predictor::new(weights);
    let mut tracker = ContextTracker::default();
    for entry in history {
        predictor.observe(&tracker.context_for(entry), &entry.command);
        tracker.record(entry);
    }
    let ctx = tracker.session_context(session_id, directory);
    (predictor, ctx)
}

/// Tracks the last commands of each session while walking history in order.
#[derive(Debug, Default)]
pub struct ContextTracker {
    sessions: HashMap<String, Context>,
}

impl ContextTracker {
    /// Context in which `entry` was run.
    #[must_use]
    pub fn context_for(&self, entry: &CommandEntry) -> Context {
        self.session_context(entry.session_id.as_ref(), &entry.directory)
    }

    /// Context for the next command in `session_id`, run from `directory`.
    #[must_use]
    pub fn session_context(&self, session_id: &str, directory: &str) -> Context {
        let mut ctx = self.sessions.get(session_id).cloned().unwrap_or_default();
        ctx.directory = directory.to_string();
        ctx
    }

    /// Record that `entry` was run.
    pub fn record(&mut self, entry: &CommandEntry) {
        let ctx = self
            .sessions
            .entry(entry.session_id.as_ref().to_string())
            .or_default();
        ctx.prev2 = ctx.prev.take();
        ctx.prev = Some(entry.command.clone());
        ctx.prev_failed = entry.exit_code.is_some_and(|rc| rc != 0);
    }
}

/// Keep commands usable for sequence modelling, in execution order.
#[must_use]
pub fn prepare_history(mut history: Vec<CommandEntry>) -> Vec<CommandEntry> {
    history.retain(|e| e.directory != IMPORTED_DIR);
    history.sort_by(|a, b| {
        a.timestamp
            .cmp(&b.timestamp)
            .then_with(|| a.id.as_i64().cmp(&b.id.as_i64()))
    });
    history
}

/// Offline evaluation result.
#[derive(Debug, Clone, Default)]
pub struct EvalReport {
    pub train: usize,
    pub test: usize,
    /// Hits within each cutoff of [`HIT_CUTOFFS`]
    pub hits: [usize; 3],
    pub reciprocal_rank_sum: f64,
    /// Test commands that had a previous command in their session
    pub with_prev: usize,
    /// Test commands already seen when predicted (ceiling for any predictor)
    pub seen_before: usize,
}

impl EvalReport {
    /// Cutoffs matching [`EvalReport::hits`].
    #[must_use]
    pub fn cutoffs() -> [usize; 3] {
        HIT_CUTOFFS
    }

    #[must_use]
    pub fn hit_rate(&self, i: usize) -> f64 {
        ratio(self.hits[i], self.test)
    }

    #[must_use]
    pub fn mrr(&self) -> f64 {
        if self.test == 0 {
            0.0
        } else {
            self.reciprocal_rank_sum / self.test as f64
        }
    }
}

/// Fraction `n / d`, or 0 when `d` is 0.
#[must_use]
pub fn ratio(n: usize, d: usize) -> f64 {
    if d == 0 { 0.0 } else { n as f64 / d as f64 }
}

/// Train on the first `train_frac` of `history` (already prepared), then
/// predict each remaining command before learning it, as live use would.
#[must_use]
pub fn evaluate(history: &[CommandEntry], train_frac: f64, weights: Weights) -> EvalReport {
    let split = (history.len() as f64 * train_frac.clamp(0.0, 1.0)) as usize;
    let max_k = HIT_CUTOFFS[HIT_CUTOFFS.len() - 1];
    let mut predictor = Predictor::new(weights);
    let mut tracker = ContextTracker::default();
    let mut report = EvalReport {
        train: split,
        test: history.len() - split,
        ..EvalReport::default()
    };

    for (i, entry) in history.iter().enumerate() {
        let ctx = tracker.context_for(entry);
        if i >= split {
            report.with_prev += usize::from(ctx.prev.is_some());
            report.seen_before += usize::from(predictor.has_seen(&entry.command));
            let predictions = predictor.predict(&ctx, max_k);
            if let Some(rank) = predictions.iter().position(|(c, _)| *c == entry.command) {
                report.reciprocal_rank_sum += 1.0 / (rank + 1) as f64;
                for (hits, &cutoff) in report.hits.iter_mut().zip(HIT_CUTOFFS.iter()) {
                    *hits += usize::from(rank < cutoff);
                }
            }
        }
        predictor.observe(&ctx, &entry.command);
        tracker.record(entry);
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{CommandId, SessionId};
    use chrono::{Duration, Utc};

    fn history(sessions: &[(&str, &str, &[&str])]) -> Vec<CommandEntry> {
        let start = Utc::now();
        let mut id = 0;
        let mut out = Vec::new();
        for (session, dir, commands) in sessions {
            for cmd in *commands {
                id += 1;
                out.push(CommandEntry {
                    id: CommandId::new(id),
                    session_id: SessionId::new((*session).to_string()),
                    command: (*cmd).to_string(),
                    timestamp: start + Duration::seconds(id),
                    directory: (*dir).to_string(),
                    redacted: false,
                    exit_code: Some(0),
                });
            }
        }
        out
    }

    fn ctx(prev: Option<&str>, dir: &str) -> Context {
        Context {
            prev: prev.map(str::to_string),
            directory: dir.to_string(),
            ..Context::default()
        }
    }

    #[test]
    fn scorer_matches_predict() {
        let mut p = Predictor::new(Weights::default());
        p.observe(&ctx(Some("a"), "/r"), "b");
        p.observe(&ctx(Some("a"), "/r"), "c");
        p.observe(&ctx(Some("a"), "/r"), "b");
        let c = ctx(Some("a"), "/r");
        let scorer = p.scorer(&c);
        for (cmd, score) in p.predict(&c, 5) {
            assert!((scorer.score(&cmd) - score).abs() < 1e-12);
        }
        assert_eq!(scorer.score("never"), 0.0);
        assert!(scorer.score("b") > scorer.score("c"));
    }

    #[test]
    fn bigram_predicts_follow_up() {
        let mut p = Predictor::new(Weights::default());
        for _ in 0..3 {
            p.observe(&ctx(Some("git add ."), "/repo"), "git commit");
            p.observe(&ctx(Some("git commit"), "/repo"), "git push");
        }
        p.observe(&ctx(None, "/repo"), "ls");
        let top = p.predict(&ctx(Some("git commit"), "/repo"), 3);
        assert_eq!(top[0].0, "git push");
    }

    #[test]
    fn template_backs_off_unseen_exact_command() {
        let mut p = Predictor::new(Weights::default());
        p.observe(&ctx(Some("cargo build --release"), "/a"), "cargo test");
        p.observe(&ctx(None, "/a"), "ls");
        p.observe(&ctx(None, "/a"), "ls");
        // Never seen exactly, but shares the "cargo build" template.
        let top = p.predict(&ctx(Some("cargo build"), "/b"), 1);
        assert_eq!(top[0].0, "cargo test");
    }

    #[test]
    fn failure_context_prefers_fixups() {
        let mut p = Predictor::new(Weights {
            failure: 5.0,
            ..Weights::default()
        });
        let failed = Context {
            prev_failed: true,
            ..ctx(Some("cargo build"), "/a")
        };
        p.observe(&failed, "cargo fix");
        p.observe(&ctx(Some("cargo build"), "/a"), "cargo run");
        p.observe(&ctx(Some("cargo build"), "/a"), "cargo run");
        assert_eq!(p.predict(&failed, 1)[0].0, "cargo fix");
        assert_eq!(
            p.predict(&ctx(Some("cargo build"), "/a"), 1)[0].0,
            "cargo run"
        );
    }

    #[test]
    fn tracker_follows_sessions_independently() {
        let h = history(&[("s1", "/a", &["a1", "a2"]), ("s2", "/b", &["b1"])]);
        let mut t = ContextTracker::default();
        for e in &h {
            t.record(e);
        }
        let c = t.session_context("s1", "/x");
        assert_eq!(c.prev.as_deref(), Some("a2"));
        assert_eq!(c.prev2.as_deref(), Some("a1"));
        assert_eq!(c.directory, "/x");
        assert_eq!(t.session_context("s2", "/b").prev.as_deref(), Some("b1"));
        assert_eq!(t.session_context("new", "/b"), ctx(None, "/b"));
    }

    #[test]
    fn evaluate_measures_repeated_sequences() {
        let seq: &[&str] = &["git pull", "cargo test", "git push"];
        let sessions: Vec<(&str, &str, &[&str])> = (0..10)
            .map(|i| {
                (
                    ["s0", "s1", "s2", "s3", "s4", "s5", "s6", "s7", "s8", "s9"][i],
                    "/r",
                    seq,
                )
            })
            .collect();
        let report = evaluate(&history(&sessions), 0.5, Weights::default());
        assert_eq!(report.train + report.test, 30);
        assert_eq!(report.seen_before, report.test);
        // Session openers have no previous command; everything after them
        // follows the learned sequence exactly.
        assert_eq!(report.with_prev, 10);
        assert!(report.hits[0] >= report.with_prev);
        assert_eq!(report.hits[1], report.test);
    }

    #[test]
    fn prepare_history_drops_imported_and_sorts() {
        let mut h = history(&[("s", "/a", &["one", "two"]), ("i", IMPORTED_DIR, &["old"])]);
        h.reverse();
        let prepared = prepare_history(h);
        let cmds: Vec<&str> = prepared.iter().map(|e| e.command.as_str()).collect();
        assert_eq!(cmds, vec!["one", "two"]);
    }

    #[test]
    fn weights_parse_roundtrip() {
        let w: Weights = "0,2,0.5,0,1,0.1".parse().unwrap();
        assert_eq!(w, Weights::default());
        assert_eq!(w.to_string().parse::<Weights>().unwrap(), w);
        assert!("1,2".parse::<Weights>().is_err());
        assert!("a,b,c,d,e,f".parse::<Weights>().is_err());
    }
}
