//! Domain models for the weighted random assignment engine.

use serde::{Deserialize, Serialize};

use super::penalty::PenaltyStrategy;

/// A record of a historical pairing between a giver and a receiver.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HistoricalPairing {
    /// The entity that was assigned to give.
    pub giver: String,
    /// The entity that was assigned to receive.
    pub receiver: String,
    /// The number of times this pairing has occurred.
    pub count: u32,
}

/// A single assignment from a giver to a receiver.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Assignment {
    /// The entity assigned to give.
    pub giver: String,
    /// The entity assigned to receive.
    pub receiver: String,
}

/// The result of generating assignments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssignmentResult {
    /// The generated assignments.
    pub assignments: Vec<Assignment>,
}

/// Configuration for an assignment generation run.
pub struct AssignmentConfig {
    /// The list of participant identifiers.
    pub participants: Vec<String>,
    /// Historical pairing records used for penalty calculation.
    pub history: Vec<HistoricalPairing>,
    /// The penalty strategy to apply.
    pub penalty_strategy: Box<dyn PenaltyStrategy>,
    /// Optional seed for deterministic random number generation.
    pub seed: Option<u64>,
}

impl AssignmentConfig {
    /// Creates a new assignment configuration.
    ///
    /// # Arguments
    ///
    /// * `participants` - The list of participant identifiers.
    /// * `history` - Historical pairing records.
    /// * `penalty_strategy` - The penalty strategy to use.
    /// * `seed` - Optional seed for deterministic output.
    pub fn new(
        participants: Vec<String>,
        history: Vec<HistoricalPairing>,
        penalty_strategy: Box<dyn PenaltyStrategy>,
        seed: Option<u64>,
    ) -> Self {
        Self {
            participants,
            history,
            penalty_strategy,
            seed,
        }
    }
}
