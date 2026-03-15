//! Property-based tests for the weighted random assignment engine.
//!
//! These tests verify invariants that must hold for all valid inputs using proptest.

use proptest::prelude::*;
use std::collections::HashSet;

use weighted_random_assignment::core::algorithm::generate_assignments;
use weighted_random_assignment::core::models::HistoricalPairing;
use weighted_random_assignment::core::penalty::{
    ExponentialPenalty, LinearPenalty, ThresholdPenalty,
};

/// Generates a vector of unique participant names.
fn unique_participants(min: usize, max: usize) -> impl Strategy<Value = Vec<String>> {
    prop::collection::hash_set("[A-Z][a-z]{2,6}", min..=max)
        .prop_map(|set| set.into_iter().collect::<Vec<_>>())
}

proptest! {
    /// Every generated assignment must pair different participants (no self-assignment).
    #[test]
    fn no_self_assignments(
        participants in unique_participants(3, 20),
        seed in 0u64..10000,
    ) {
        let strategy = LinearPenalty::new(1.0);
        let result = generate_assignments(&participants, &[], &strategy, Some(seed));

        if let Ok(assignments) = result {
            for assignment in &assignments {
                prop_assert_ne!(
                    &assignment.giver,
                    &assignment.receiver,
                    "self-assignment: {} -> {}",
                    assignment.giver,
                    assignment.receiver
                );
            }
        }
    }

    /// The result must be a valid permutation: each participant appears exactly
    /// once as giver and exactly once as receiver.
    #[test]
    fn valid_permutation(
        participants in unique_participants(3, 20),
        seed in 0u64..10000,
    ) {
        let strategy = LinearPenalty::new(1.0);
        let result = generate_assignments(&participants, &[], &strategy, Some(seed));

        if let Ok(assignments) = result {
            prop_assert_eq!(assignments.len(), participants.len());

            let givers: HashSet<&str> = assignments.iter().map(|a| a.giver.as_str()).collect();
            let receivers: HashSet<&str> = assignments.iter().map(|a| a.receiver.as_str()).collect();
            let expected: HashSet<&str> = participants.iter().map(|p| p.as_str()).collect();

            prop_assert_eq!(&givers, &expected, "not all participants are givers");
            prop_assert_eq!(&receivers, &expected, "not all participants are receivers");
        }
    }

    /// With a very high penalty factor, repeated pairings should be less likely.
    /// We test this statistically over multiple seeds.
    #[test]
    fn penalty_bias_reduces_repeats(
        seed_base in 0u64..1000,
    ) {
        let participants: Vec<String> = vec!["A", "B", "C", "D", "E"]
            .into_iter()
            .map(String::from)
            .collect();

        let history = vec![
            HistoricalPairing {
                giver: "A".to_string(),
                receiver: "B".to_string(),
                count: 20,
            },
        ];

        let strategy = LinearPenalty::new(10.0);

        let mut repeat_count = 0;
        let trials = 200;

        for i in 0..trials {
            let seed = seed_base * 10000 + i;
            if let Ok(assignments) = generate_assignments(&participants, &history, &strategy, Some(seed)) {
                if assignments.iter().any(|a| a.giver == "A" && a.receiver == "B") {
                    repeat_count += 1;
                }
            }
        }

        // With penalty_factor=10 and count=20, weight is 1/(1+200) ≈ 0.005
        // vs 1.0 for other options. So A->B should be very rare.
        // Due to derangement constraints the actual rate can be slightly higher,
        // but it should still be well under 20%.
        let frequency = repeat_count as f64 / trials as f64;
        prop_assert!(
            frequency < 0.20,
            "A->B occurred {} times out of {} ({:.1}%); expected much less with heavy penalty",
            repeat_count, trials, frequency * 100.0
        );
    }

    /// The algorithm must work correctly with exactly 2 participants.
    /// The only valid derangement for 2 participants is a swap.
    #[test]
    fn two_participants_always_swap(
        seed in 0u64..10000,
    ) {
        let participants = vec!["X".to_string(), "Y".to_string()];
        let strategy = LinearPenalty::new(1.0);
        let result = generate_assignments(&participants, &[], &strategy, Some(seed)).unwrap();

        prop_assert_eq!(result.len(), 2);
        // For two participants, the only derangement is the swap
        let x_assignment = result.iter().find(|a| a.giver == "X").unwrap();
        let y_assignment = result.iter().find(|a| a.giver == "Y").unwrap();
        prop_assert_eq!(&x_assignment.receiver, "Y");
        prop_assert_eq!(&y_assignment.receiver, "X");
    }

    /// The result must be a valid derangement with the exponential penalty strategy.
    #[test]
    fn exponential_strategy_valid_derangement(
        participants in unique_participants(3, 15),
        seed in 0u64..5000,
        decay_rate in 0.1f64..0.9,
    ) {
        let strategy = ExponentialPenalty::new(decay_rate);
        let result = generate_assignments(&participants, &[], &strategy, Some(seed));

        if let Ok(assignments) = result {
            for assignment in &assignments {
                prop_assert_ne!(&assignment.giver, &assignment.receiver);
            }
            prop_assert_eq!(assignments.len(), participants.len());
        }
    }

    /// The result must be a valid derangement with the threshold penalty strategy.
    #[test]
    fn threshold_strategy_valid_derangement(
        participants in unique_participants(3, 15),
        seed in 0u64..5000,
        threshold in 1u32..5,
    ) {
        let strategy = ThresholdPenalty::new(threshold, 0.1);
        let result = generate_assignments(&participants, &[], &strategy, Some(seed));

        if let Ok(assignments) = result {
            for assignment in &assignments {
                prop_assert_ne!(&assignment.giver, &assignment.receiver);
            }
            prop_assert_eq!(assignments.len(), participants.len());
        }
    }

    /// Determinism: the same seed must always produce the same result.
    #[test]
    fn deterministic_output(
        participants in unique_participants(3, 10),
        seed in 0u64..10000,
    ) {
        let strategy1 = LinearPenalty::new(1.0);
        let strategy2 = LinearPenalty::new(1.0);
        let result1 = generate_assignments(&participants, &[], &strategy1, Some(seed));
        let result2 = generate_assignments(&participants, &[], &strategy2, Some(seed));

        match (result1, result2) {
            (Ok(a1), Ok(a2)) => prop_assert_eq!(a1, a2),
            (Err(_), Err(_)) => {} // Both failed, that's consistent
            _ => prop_assert!(false, "inconsistent results for same seed"),
        }
    }
}
