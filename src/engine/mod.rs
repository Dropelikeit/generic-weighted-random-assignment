//! Engine layer: orchestration, validation, and generation logic.
//!
//! The engine layer sits between the core algorithm and the external interfaces
//! (API, CLI). It handles input validation, configuration, and coordinates the
//! assignment generation process.
//!
//! **Important:** All external entry points (API, CLI) must route through
//! [`AssignmentEngine::generate`] rather than calling
//! [`crate::core::algorithm::generate_assignments`] directly. The engine's
//! [`validate`](AssignmentEngine::validate) method enforces invariants
//! (minimum participants, non-empty names, valid history references) that the
//! core algorithm assumes but does not re-check.

use std::collections::HashSet;

use crate::core::algorithm::{generate_assignments, AlgorithmError};
use crate::core::models::{AssignmentConfig, AssignmentResult};

/// Errors that can occur in the engine layer.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    /// The algorithm encountered an error.
    #[error("algorithm error: {0}")]
    Algorithm(#[from] AlgorithmError),

    /// Input validation failed.
    #[error("validation error: {0}")]
    Validation(String),
}

/// The assignment engine orchestrates the generation of weighted random assignments.
///
/// It validates inputs, applies configuration, and delegates to the core algorithm.
#[derive(Debug, Default)]
pub struct AssignmentEngine;

impl AssignmentEngine {
    /// Creates a new assignment engine instance.
    pub fn new() -> Self {
        Self
    }

    /// Generates assignments based on the provided configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - The assignment configuration including participants, history, and penalty strategy.
    ///
    /// # Returns
    ///
    /// An `AssignmentResult` containing the generated assignments, or an error.
    ///
    /// # Errors
    ///
    /// Returns `EngineError::Validation` if the input is invalid, or
    /// `EngineError::Algorithm` if the algorithm fails to produce a valid result.
    pub fn generate(&self, config: AssignmentConfig) -> Result<AssignmentResult, EngineError> {
        self.validate(&config)?;

        tracing::info!(
            participants = config.participants.len(),
            history_records = config.history.len(),
            strategy = config.penalty_strategy.name(),
            seed = ?config.seed,
            "generating assignments"
        );

        let assignments = generate_assignments(
            &config.participants,
            &config.history,
            config.penalty_strategy.as_ref(),
            config.seed,
        )?;

        tracing::info!(
            assignments = assignments.len(),
            "assignments generated successfully"
        );

        Ok(AssignmentResult { assignments })
    }

    /// Validates the assignment configuration.
    fn validate(&self, config: &AssignmentConfig) -> Result<(), EngineError> {
        if config.participants.len() < 2 {
            return Err(EngineError::Validation(
                "need at least 2 participants".to_string(),
            ));
        }

        // Validate that participant names are non-empty
        for (i, p) in config.participants.iter().enumerate() {
            if p.trim().is_empty() {
                return Err(EngineError::Validation(format!(
                    "participant at index {} has an empty name",
                    i
                )));
            }
        }

        // Validate history references existing participants (O(1) lookup)
        let participant_set: HashSet<&str> =
            config.participants.iter().map(|s| s.as_str()).collect();

        for record in &config.history {
            if !participant_set.contains(record.giver.as_str()) {
                return Err(EngineError::Validation(format!(
                    "history references unknown giver: {}",
                    record.giver
                )));
            }
            if !participant_set.contains(record.receiver.as_str()) {
                return Err(EngineError::Validation(format!(
                    "history references unknown receiver: {}",
                    record.receiver
                )));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::models::HistoricalPairing;
    use crate::core::penalty::LinearPenalty;

    fn make_config(
        participants: Vec<&str>,
        history: Vec<HistoricalPairing>,
        penalty_factor: f64,
        seed: Option<u64>,
    ) -> AssignmentConfig {
        AssignmentConfig::new(
            participants.into_iter().map(String::from).collect(),
            history,
            Box::new(LinearPenalty::new(penalty_factor)),
            seed,
        )
    }

    #[test]
    fn test_engine_basic_generation() {
        let engine = AssignmentEngine::new();
        let config = make_config(vec!["A", "B", "C", "D"], vec![], 1.0, Some(42));
        let result = engine.generate(config).unwrap();
        assert_eq!(result.assignments.len(), 4);
    }

    #[test]
    fn test_engine_empty_participants() {
        let engine = AssignmentEngine::new();
        let config = make_config(vec![], vec![], 1.0, Some(42));
        let result = engine.generate(config);
        assert!(matches!(result, Err(EngineError::Validation(_))));
    }

    #[test]
    fn test_engine_single_participant() {
        let engine = AssignmentEngine::new();
        let config = make_config(vec!["A"], vec![], 1.0, Some(42));
        let result = engine.generate(config);
        assert!(matches!(result, Err(EngineError::Validation(_))));
    }

    #[test]
    fn test_engine_empty_participant_name() {
        let engine = AssignmentEngine::new();
        let config = make_config(vec!["A", "", "C"], vec![], 1.0, Some(42));
        let result = engine.generate(config);
        assert!(matches!(result, Err(EngineError::Validation(_))));
    }

    #[test]
    fn test_engine_invalid_history_giver() {
        let engine = AssignmentEngine::new();
        let history = vec![HistoricalPairing {
            giver: "X".to_string(),
            receiver: "A".to_string(),
            count: 1,
        }];
        let config = make_config(vec!["A", "B", "C"], history, 1.0, Some(42));
        let result = engine.generate(config);
        assert!(matches!(result, Err(EngineError::Validation(_))));
    }

    #[test]
    fn test_engine_invalid_history_receiver() {
        let engine = AssignmentEngine::new();
        let history = vec![HistoricalPairing {
            giver: "A".to_string(),
            receiver: "Z".to_string(),
            count: 1,
        }];
        let config = make_config(vec!["A", "B", "C"], history, 1.0, Some(42));
        let result = engine.generate(config);
        assert!(matches!(result, Err(EngineError::Validation(_))));
    }

    #[test]
    fn test_engine_with_history() {
        let engine = AssignmentEngine::new();
        let history = vec![HistoricalPairing {
            giver: "A".to_string(),
            receiver: "B".to_string(),
            count: 5,
        }];
        let config = make_config(vec!["A", "B", "C", "D"], history, 2.0, Some(42));
        let result = engine.generate(config).unwrap();
        assert_eq!(result.assignments.len(), 4);

        // Verify no self-assignments
        for a in &result.assignments {
            assert_ne!(a.giver, a.receiver);
        }
    }
}
