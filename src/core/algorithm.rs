//! Core weighted random assignment algorithm with derangement support.
//!
//! This module implements the main assignment algorithm that produces valid
//! derangements (permutations with no fixed points) using weighted random
//! selection with historical penalty adjustments.

use std::collections::HashMap;

use rand::prelude::IndexedRandom;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};

use super::models::{Assignment, HistoricalPairing};
use super::penalty::PenaltyStrategy;

/// Maximum number of retry attempts before falling back to a different approach.
const MAX_RETRIES: usize = 100;

/// Errors that can occur during assignment generation.
#[derive(Debug, thiserror::Error)]
pub enum AlgorithmError {
    /// Not enough participants to form valid assignments.
    #[error("need at least 2 participants for assignment, got {0}")]
    InsufficientParticipants(usize),

    /// Failed to produce a valid derangement within retry limits.
    #[error("failed to generate valid derangement after {0} attempts")]
    DerangementFailed(usize),

    /// Duplicate participant identifiers found.
    #[error("duplicate participant found: {0}")]
    DuplicateParticipant(String),
}

/// Builds a lookup map from historical pairings for O(1) access.
///
/// When multiple entries exist for the same (giver, receiver) pair, their
/// counts are summed. This allows history to be provided as separate records
/// (e.g., one per year) that are accumulated into a total.
///
/// The map uses borrowed string slices from the input to avoid allocation.
///
/// # Arguments
///
/// * `history` - Slice of historical pairings to index.
///
/// # Returns
///
/// A map from (&giver, &receiver) to total occurrence count.
pub fn build_history_map(history: &[HistoricalPairing]) -> HashMap<(&str, &str), u32> {
    let mut map = HashMap::new();
    for record in history {
        let key = (record.giver.as_str(), record.receiver.as_str());
        let count = map.entry(key).or_insert(0u32);
        *count = (*count).saturating_add(record.count);
    }
    map
}

/// Calculates the adjusted weight for a potential (giver, receiver) pairing.
///
/// # Arguments
///
/// * `giver` - The giver participant identifier.
/// * `receiver` - The receiver participant identifier.
/// * `history_map` - Map of historical pairing counts (borrowed keys).
/// * `strategy` - The penalty strategy to apply.
/// * `base_weight` - The base weight before penalty adjustment.
///
/// # Returns
///
/// The adjusted weight for this pairing.
pub fn calculate_weight(
    giver: &str,
    receiver: &str,
    history_map: &HashMap<(&str, &str), u32>,
    strategy: &dyn PenaltyStrategy,
    base_weight: f64,
) -> f64 {
    let count = history_map.get(&(giver, receiver)).copied().unwrap_or(0);
    strategy.adjusted_weight(base_weight, count)
}

/// Selects a receiver from candidates using weighted random selection.
///
/// # Arguments
///
/// * `candidates` - Available receiver candidates with their weights.
/// * `rng` - The random number generator to use.
///
/// # Returns
///
/// The index of the selected candidate, or `None` if all weights are zero.
fn weighted_select(candidates: &[(usize, f64)], rng: &mut StdRng) -> Option<usize> {
    let total_weight: f64 = candidates.iter().map(|(_, w)| w).sum();
    if total_weight.is_nan() || total_weight <= 0.0 {
        // Fallback: uniform random selection when weights are zero or corrupt
        return candidates.choose(rng).map(|(idx, _)| *idx);
    }

    let mut threshold = rng.random::<f64>() * total_weight;
    for &(idx, weight) in candidates {
        threshold -= weight;
        if threshold < 0.0 {
            return Some(idx);
        }
    }

    // Fallback for floating-point edge cases
    candidates.last().map(|(idx, _)| *idx)
}

/// Generates assignments using the weighted derangement algorithm.
///
/// The algorithm works as follows:
/// 1. Shuffle participants to avoid ordering bias.
/// 2. For each participant (giver), calculate weights for all possible receivers.
/// 3. Use weighted random selection to pick a receiver.
/// 4. Validate that the result is a valid derangement.
/// 5. Retry with a fresh shuffle if a dead-end is reached.
///
/// # Arguments
///
/// * `participants` - List of participant identifiers.
/// * `history` - Historical pairing records.
/// * `strategy` - The penalty strategy to apply.
/// * `seed` - Optional seed for deterministic output.
///
/// # Returns
///
/// A vector of assignments forming a valid derangement, or an error.
pub fn generate_assignments(
    participants: &[String],
    history: &[HistoricalPairing],
    strategy: &dyn PenaltyStrategy,
    seed: Option<u64>,
) -> Result<Vec<Assignment>, AlgorithmError> {
    let n = participants.len();
    if n < 2 {
        return Err(AlgorithmError::InsufficientParticipants(n));
    }

    // Check for duplicates
    let mut seen = HashMap::new();
    for p in participants {
        if seen.insert(p.as_str(), ()).is_some() {
            return Err(AlgorithmError::DuplicateParticipant(p.clone()));
        }
    }

    let history_map = build_history_map(history);
    let base_weight = 1.0;

    let mut rng = match seed {
        Some(s) => StdRng::seed_from_u64(s),
        None => StdRng::from_os_rng(),
    };

    for attempt in 0..MAX_RETRIES {
        match try_generate(participants, &history_map, strategy, base_weight, &mut rng) {
            Some(assignments) => return Ok(assignments),
            None => {
                tracing::debug!(attempt, "derangement attempt failed, retrying");
            }
        }

        // After many failed attempts, reseed deterministically based on the
        // original seed and attempt number to explore different orderings while
        // preserving reproducibility. Each late attempt gets a single-shot with
        // a fresh, deterministically-derived seed.
        if attempt >= MAX_RETRIES / 2 {
            let reseed = match seed {
                Some(s) => s
                    .wrapping_add(attempt as u64)
                    .wrapping_mul(6364136223846793005),
                None => rng.random(),
            };
            rng = StdRng::seed_from_u64(reseed);
        }
    }

    Err(AlgorithmError::DerangementFailed(MAX_RETRIES))
}

/// Attempts a single generation of a valid derangement.
///
/// Uses a greedy weighted selection approach with look-ahead to avoid dead-ends.
///
/// # Returns
///
/// `Some(assignments)` if a valid derangement was produced, `None` otherwise.
fn try_generate(
    participants: &[String],
    history_map: &HashMap<(&str, &str), u32>,
    strategy: &dyn PenaltyStrategy,
    base_weight: f64,
    rng: &mut StdRng,
) -> Option<Vec<Assignment>> {
    let n = participants.len();

    // Create a shuffled order of indices to process
    let mut order: Vec<usize> = (0..n).collect();
    order.shuffle(rng);

    // Track which receivers are still available
    let mut available: Vec<bool> = vec![true; n];
    let mut assignments: Vec<Option<usize>> = vec![None; n];

    for &giver_idx in &order {
        let giver = &participants[giver_idx];

        // Build weighted candidate list (excluding self and already-assigned)
        let candidates: Vec<(usize, f64)> = (0..n)
            .filter(|&recv_idx| recv_idx != giver_idx && available[recv_idx])
            .map(|recv_idx| {
                let receiver = &participants[recv_idx];
                let weight = calculate_weight(giver, receiver, history_map, strategy, base_weight);
                (recv_idx, weight)
            })
            .collect();

        if candidates.is_empty() {
            return None; // Dead-end
        }

        // Check for forced assignments: if any remaining unassigned giver
        // has only one possible receiver, don't steal that receiver
        let candidates =
            apply_lookahead(&candidates, giver_idx, &order, &assignments, &available, n);

        if candidates.is_empty() {
            return None;
        }

        let selected = weighted_select(&candidates, rng)?;
        assignments[giver_idx] = Some(selected);
        available[selected] = false;
    }

    // Build the result
    let result: Vec<Assignment> = assignments
        .iter()
        .enumerate()
        .filter_map(|(giver_idx, recv_idx)| {
            recv_idx.map(|r| Assignment {
                giver: participants[giver_idx].clone(),
                receiver: participants[r].clone(),
            })
        })
        .collect();

    if result.len() == n {
        Some(result)
    } else {
        None
    }
}

/// Applies look-ahead logic to prevent dead-ends.
///
/// Removes candidates from the current selection if assigning them would leave
/// another unassigned giver with no valid receivers.
///
/// **Note:** This is a single-step heuristic, not a complete constraint solver.
/// It verifies that each remaining unassigned giver has at least one available
/// receiver after the simulated selection, but it does not detect conflicts where
/// multiple givers compete for the same sole remaining receiver. In such cases
/// the attempt will fail and the outer retry loop will try again with a fresh
/// shuffle. In practice this is sufficient for all but extreme edge cases.
///
/// **Performance:** The current implementation has O(C * U * N) complexity per
/// giver (C = candidates, U = unassigned givers, N = participants). For very
/// large participant counts (thousands+), consider precomputing per-giver
/// available-receiver counts and maintaining them incrementally. For the typical
/// use case (tens to low hundreds of participants) this is not a bottleneck.
fn apply_lookahead(
    candidates: &[(usize, f64)],
    current_giver: usize,
    order: &[usize],
    assignments: &[Option<usize>],
    available: &[bool],
    n: usize,
) -> Vec<(usize, f64)> {
    // Find unassigned givers (excluding current)
    let unassigned_givers: Vec<usize> = order
        .iter()
        .filter(|&&idx| idx != current_giver && assignments[idx].is_none())
        .copied()
        .collect();

    candidates
        .iter()
        .filter(|&&(recv_idx, _)| {
            // Simulate taking this receiver
            // Check if all remaining unassigned givers still have at least one option
            for &other_giver in &unassigned_givers {
                let remaining_options = (0..n)
                    .filter(|&r| {
                        r != other_giver && available[r] && r != recv_idx // this one would be taken
                    })
                    .count();
                if remaining_options == 0 {
                    return false;
                }
            }
            true
        })
        .copied()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::penalty::LinearPenalty;

    fn test_participants(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("P{}", i)).collect()
    }

    #[test]
    fn test_insufficient_participants() {
        let strategy = LinearPenalty::new(1.0);
        let result = generate_assignments(&["A".to_string()], &[], &strategy, Some(42));
        assert!(matches!(
            result,
            Err(AlgorithmError::InsufficientParticipants(1))
        ));
    }

    #[test]
    fn test_empty_participants() {
        let strategy = LinearPenalty::new(1.0);
        let result = generate_assignments(&[], &[], &strategy, Some(42));
        assert!(matches!(
            result,
            Err(AlgorithmError::InsufficientParticipants(0))
        ));
    }

    #[test]
    fn test_duplicate_participants() {
        let strategy = LinearPenalty::new(1.0);
        let participants = vec!["A".to_string(), "B".to_string(), "A".to_string()];
        let result = generate_assignments(&participants, &[], &strategy, Some(42));
        assert!(matches!(
            result,
            Err(AlgorithmError::DuplicateParticipant(_))
        ));
    }

    #[test]
    fn test_two_participants() {
        let strategy = LinearPenalty::new(1.0);
        let participants = vec!["A".to_string(), "B".to_string()];
        let result = generate_assignments(&participants, &[], &strategy, Some(42)).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].giver, "A");
        assert_eq!(result[0].receiver, "B");
        assert_eq!(result[1].giver, "B");
        assert_eq!(result[1].receiver, "A");
    }

    #[test]
    fn test_no_self_assignment() {
        let strategy = LinearPenalty::new(1.0);
        let participants = test_participants(10);
        for seed in 0..50 {
            let result = generate_assignments(&participants, &[], &strategy, Some(seed)).unwrap();
            for assignment in &result {
                assert_ne!(
                    assignment.giver, assignment.receiver,
                    "self-assignment detected: {} -> {}",
                    assignment.giver, assignment.receiver
                );
            }
        }
    }

    #[test]
    fn test_valid_permutation() {
        let strategy = LinearPenalty::new(1.0);
        let participants = test_participants(8);
        let result = generate_assignments(&participants, &[], &strategy, Some(42)).unwrap();

        assert_eq!(result.len(), participants.len());

        // Each participant appears exactly once as giver
        let givers: Vec<&str> = result.iter().map(|a| a.giver.as_str()).collect();
        let mut sorted_givers = givers.clone();
        sorted_givers.sort();
        sorted_givers.dedup();
        assert_eq!(sorted_givers.len(), participants.len());

        // Each participant appears exactly once as receiver
        let receivers: Vec<&str> = result.iter().map(|a| a.receiver.as_str()).collect();
        let mut sorted_receivers = receivers.clone();
        sorted_receivers.sort();
        sorted_receivers.dedup();
        assert_eq!(sorted_receivers.len(), participants.len());
    }

    #[test]
    fn test_deterministic_with_seed() {
        let strategy = LinearPenalty::new(1.0);
        let participants = test_participants(6);
        let result1 = generate_assignments(&participants, &[], &strategy, Some(12345)).unwrap();
        let strategy2 = LinearPenalty::new(1.0);
        let result2 = generate_assignments(&participants, &[], &strategy2, Some(12345)).unwrap();
        assert_eq!(result1, result2);
    }

    #[test]
    fn test_history_map_building() {
        let history = vec![
            HistoricalPairing {
                giver: "A".to_string(),
                receiver: "B".to_string(),
                count: 3,
            },
            HistoricalPairing {
                giver: "C".to_string(),
                receiver: "D".to_string(),
                count: 1,
            },
        ];
        let map = build_history_map(&history);
        assert_eq!(map.get(&("A", "B")), Some(&3));
        assert_eq!(map.get(&("C", "D")), Some(&1));
        assert_eq!(map.get(&("A", "C")), None);
    }

    #[test]
    fn test_history_map_sums_duplicates() {
        let history = vec![
            HistoricalPairing {
                giver: "A".to_string(),
                receiver: "B".to_string(),
                count: 3,
            },
            HistoricalPairing {
                giver: "A".to_string(),
                receiver: "B".to_string(),
                count: 2,
            },
        ];
        let map = build_history_map(&history);
        // Duplicate entries should be summed: 3 + 2 = 5
        assert_eq!(map.get(&("A", "B")), Some(&5));
    }

    #[test]
    fn test_weight_calculation() {
        let strategy = LinearPenalty::new(1.0);
        let mut history_map = HashMap::new();
        history_map.insert(("A", "B"), 2u32);

        // A->B should have reduced weight
        let weight_ab = calculate_weight("A", "B", &history_map, &strategy, 1.0);
        assert!((weight_ab - 1.0 / 3.0).abs() < f64::EPSILON);

        // A->C should have full weight (no history)
        let weight_ac = calculate_weight("A", "C", &history_map, &strategy, 1.0);
        assert!((weight_ac - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_penalty_reduces_pairing_probability() {
        let strategy = LinearPenalty::new(5.0);
        let participants = vec![
            "A".to_string(),
            "B".to_string(),
            "C".to_string(),
            "D".to_string(),
        ];
        let history = vec![HistoricalPairing {
            giver: "A".to_string(),
            receiver: "B".to_string(),
            count: 10,
        }];

        // Run many times and count how often A->B occurs
        let mut ab_count = 0;
        let trials = 500;
        for seed in 0..trials {
            let result =
                generate_assignments(&participants, &history, &strategy, Some(seed)).unwrap();
            if let Some(a_assignment) = result.iter().find(|a| a.giver == "A") {
                if a_assignment.receiver == "B" {
                    ab_count += 1;
                }
            }
        }

        // With heavy penalty, A->B should occur much less than 1/3 of the time
        let frequency = ab_count as f64 / trials as f64;
        assert!(
            frequency < 0.15,
            "A->B frequency {} is too high with heavy penalty",
            frequency
        );
    }
}
