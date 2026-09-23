//! Next-command prediction handler

use crate::cli::args::PredictArgs;
use crate::cli::{CliApp, HistoryBackend};
use crate::error::{Error, Result};
use crate::predict::{self, EvalReport, Weights};
use std::env;

pub fn handle_predict(app: &mut CliApp, args: &PredictArgs) -> Result<()> {
    let HistoryBackend::Database(mgr) = &app.backend else {
        return Err(Error::custom(
            "Prediction requires the database backend. Remove --use-file flag.",
        ));
    };
    let history = predict::prepare_history(mgr.get_all_commands()?);

    if args.eval {
        if !(args.train_frac > 0.0 && args.train_frac < 1.0) {
            return Err(Error::custom("--train-frac must be between 0 and 1"));
        }
        run_eval(&history, args);
        return Ok(());
    }

    let cwd = env::current_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    let session_id = env::var("ZAM_SESSION_ID").unwrap_or_default();
    let (predictor, ctx) = predict::train(
        &history,
        args.weights.unwrap_or_default(),
        &session_id,
        &cwd,
    );

    for (command, score) in predictor.predict(&ctx, args.count) {
        if app.verbose {
            println!("{score:6.3}  {command}");
        } else {
            println!("{command}");
        }
    }
    Ok(())
}

fn run_eval(history: &[crate::database::CommandEntry], args: &PredictArgs) {
    let mut runs: Vec<(String, Weights)> = Weights::presets()
        .into_iter()
        .map(|(name, w)| (name.to_string(), w))
        .collect();
    if let Some(w) = args.weights {
        runs.push((format!("custom {w}"), w));
    }

    let cutoffs = EvalReport::cutoffs();
    let header: Vec<String> = cutoffs.iter().map(|k| format!("hit@{k}")).collect();
    println!(
        "{:<28} {:>7} {:>7} {:>7} {:>7}",
        "predictor", header[0], header[1], header[2], "MRR"
    );

    let mut last = EvalReport::default();
    for (name, weights) in runs {
        let report = predict::evaluate(history, args.train_frac, weights);
        println!(
            "{:<28} {:>6.1}% {:>6.1}% {:>6.1}% {:>7.3}",
            name,
            report.hit_rate(0) * 100.0,
            report.hit_rate(1) * 100.0,
            report.hit_rate(2) * 100.0,
            report.mrr(),
        );
        last = report;
    }

    println!(
        "\ntrain {} / test {} commands; {:.1}% of test had a previous command in session, \
         {:.1}% were seen before (ceiling for any predictor)",
        last.train,
        last.test,
        predict::ratio(last.with_prev, last.test) * 100.0,
        predict::ratio(last.seen_before, last.test) * 100.0,
    );
}
