//! CLI tool for the weighted random assignment engine.
//!
//! Allows manual testing and scripting of assignment generation from the command line.
//!
//! # Examples
//!
//! ```bash
//! wra generate --participants A,B,C,D --penalty-factor 1.0
//! wra generate --participants A,B,C,D --history history.json --penalty-factor 2.0
//! wra generate --participants A,B,C,D --strategy exponential --decay-rate 0.5
//! ```

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};

use weighted_random_assignment::core::models::{AssignmentConfig, HistoricalPairing};
use weighted_random_assignment::core::penalty::{
    ExponentialPenalty, LinearPenalty, PenaltyStrategy, ThresholdPenalty,
};
use weighted_random_assignment::engine::AssignmentEngine;
use weighted_random_assignment::infra::logging;

#[derive(Clone, ValueEnum)]
enum StrategyKind {
    Linear,
    Exponential,
    Threshold,
}

/// Weighted random assignment engine CLI.
#[derive(Parser)]
#[command(
    name = "wra",
    about = "Generic weighted random assignment engine with historical penalty support",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate random assignments.
    Generate {
        /// Comma-separated list of participant names.
        #[arg(long, value_delimiter = ',')]
        participants: Vec<String>,

        /// Path to a JSON file containing historical pairings.
        #[arg(long)]
        history: Option<PathBuf>,

        /// Penalty strategy to use.
        #[arg(long, value_enum, default_value_t = StrategyKind::Linear)]
        strategy: StrategyKind,

        /// Penalty factor for weight reduction [linear strategy only].
        #[arg(long, default_value = "1.0")]
        penalty_factor: f64,

        /// Decay rate per historical occurrence [exponential strategy only].
        #[arg(long, default_value = "0.5")]
        decay_rate: f64,

        /// Historical count before penalty activates [threshold strategy only].
        #[arg(long, default_value = "2")]
        threshold: u32,

        /// Weight multiplier when threshold is exceeded [threshold strategy only].
        #[arg(long, default_value = "0.1")]
        reduced_weight_factor: f64,

        /// Optional seed for deterministic output.
        #[arg(long)]
        seed: Option<u64>,
    },
}

fn main() -> Result<()> {
    logging::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Generate {
            participants,
            history,
            strategy,
            penalty_factor,
            decay_rate,
            threshold,
            reduced_weight_factor,
            seed,
        } => {
            let history_data = match history {
                Some(path) => {
                    let content = std::fs::read_to_string(&path).with_context(|| {
                        format!("failed to read history file: {}", path.display())
                    })?;
                    serde_json::from_str::<Vec<HistoricalPairing>>(&content)
                        .with_context(|| "failed to parse history JSON")?
                }
                None => vec![],
            };

            let penalty_strategy: Box<dyn PenaltyStrategy> = match strategy {
                StrategyKind::Linear => {
                    anyhow::ensure!(
                        penalty_factor >= 0.0 && penalty_factor.is_finite(),
                        "penalty_factor must be non-negative and finite, got {}",
                        penalty_factor
                    );
                    Box::new(LinearPenalty::new(penalty_factor))
                }
                StrategyKind::Exponential => {
                    anyhow::ensure!(
                        (0.0..=1.0).contains(&decay_rate),
                        "decay_rate must be between 0.0 and 1.0, got {}",
                        decay_rate
                    );
                    Box::new(ExponentialPenalty::new(decay_rate))
                }
                StrategyKind::Threshold => {
                    anyhow::ensure!(
                        (0.0..=1.0).contains(&reduced_weight_factor),
                        "reduced_weight_factor must be between 0.0 and 1.0, got {}",
                        reduced_weight_factor
                    );
                    Box::new(ThresholdPenalty::new(threshold, reduced_weight_factor))
                }
            };

            let config = AssignmentConfig::new(participants, history_data, penalty_strategy, seed);
            let engine = AssignmentEngine::new();
            let result = engine
                .generate(config)
                .context("assignment generation failed")?;

            let json =
                serde_json::to_string_pretty(&result).context("failed to serialize result")?;
            println!("{}", json);
        }
    }

    Ok(())
}
