//! Integration tests for the assignment engine.
//!
//! Tests the full pipeline from configuration to result.

use weighted_random_assignment::core::models::{AssignmentConfig, HistoricalPairing};
use weighted_random_assignment::core::penalty::{
    ExponentialPenalty, LinearPenalty, ThresholdPenalty,
};
use weighted_random_assignment::engine::AssignmentEngine;

#[test]
fn test_full_pipeline_linear() {
    let participants: Vec<String> = vec!["Alice", "Bob", "Charlie", "Diana"]
        .into_iter()
        .map(String::from)
        .collect();

    let history = vec![
        HistoricalPairing {
            giver: "Alice".to_string(),
            receiver: "Bob".to_string(),
            count: 3,
        },
        HistoricalPairing {
            giver: "Bob".to_string(),
            receiver: "Charlie".to_string(),
            count: 1,
        },
    ];

    let strategy = LinearPenalty::new(2.0);
    let config = AssignmentConfig::new(participants.clone(), history, Box::new(strategy), Some(42));

    let engine = AssignmentEngine::new();
    let result = engine.generate(config).unwrap();

    assert_eq!(result.assignments.len(), 4);

    // Verify all participants are givers and receivers
    for p in &participants {
        assert!(result.assignments.iter().any(|a| &a.giver == p));
        assert!(result.assignments.iter().any(|a| &a.receiver == p));
    }

    // No self-assignments
    for a in &result.assignments {
        assert_ne!(a.giver, a.receiver);
    }
}

#[test]
fn test_full_pipeline_exponential() {
    let participants: Vec<String> = vec!["A", "B", "C", "D", "E"]
        .into_iter()
        .map(String::from)
        .collect();

    let strategy = ExponentialPenalty::new(0.5);
    let config = AssignmentConfig::new(participants.clone(), vec![], Box::new(strategy), Some(123));

    let engine = AssignmentEngine::new();
    let result = engine.generate(config).unwrap();

    assert_eq!(result.assignments.len(), 5);

    for a in &result.assignments {
        assert_ne!(a.giver, a.receiver);
    }
}

#[test]
fn test_full_pipeline_threshold() {
    let participants: Vec<String> = vec!["X", "Y", "Z"].into_iter().map(String::from).collect();

    let history = vec![HistoricalPairing {
        giver: "X".to_string(),
        receiver: "Y".to_string(),
        count: 5,
    }];

    let strategy = ThresholdPenalty::new(2, 0.01);
    let config = AssignmentConfig::new(participants.clone(), history, Box::new(strategy), Some(99));

    let engine = AssignmentEngine::new();
    let result = engine.generate(config).unwrap();

    assert_eq!(result.assignments.len(), 3);

    for a in &result.assignments {
        assert_ne!(a.giver, a.receiver);
    }
}

#[test]
fn test_large_group() {
    let participants: Vec<String> = (0..50).map(|i| format!("Person_{}", i)).collect();
    let strategy = LinearPenalty::new(1.0);
    let config = AssignmentConfig::new(participants.clone(), vec![], Box::new(strategy), Some(42));

    let engine = AssignmentEngine::new();
    let result = engine.generate(config).unwrap();

    assert_eq!(result.assignments.len(), 50);

    for a in &result.assignments {
        assert_ne!(a.giver, a.receiver);
    }
}

#[test]
fn test_serialization_roundtrip() {
    let participants: Vec<String> = vec!["A", "B", "C"].into_iter().map(String::from).collect();
    let strategy = LinearPenalty::new(1.0);
    let config = AssignmentConfig::new(participants, vec![], Box::new(strategy), Some(42));

    let engine = AssignmentEngine::new();
    let result = engine.generate(config).unwrap();

    // Serialize to JSON and back
    let json = serde_json::to_string(&result).unwrap();
    let deserialized: weighted_random_assignment::core::models::AssignmentResult =
        serde_json::from_str(&json).unwrap();

    assert_eq!(result.assignments.len(), deserialized.assignments.len());
    for (a, b) in result
        .assignments
        .iter()
        .zip(deserialized.assignments.iter())
    {
        assert_eq!(a.giver, b.giver);
        assert_eq!(a.receiver, b.receiver);
    }
}

#[test]
fn test_heavy_history_still_produces_valid_result() {
    let participants: Vec<String> = vec!["A", "B", "C", "D"]
        .into_iter()
        .map(String::from)
        .collect();

    // Every possible pairing has high history
    let mut history = vec![];
    for g in &participants {
        for r in &participants {
            if g != r {
                history.push(HistoricalPairing {
                    giver: g.clone(),
                    receiver: r.clone(),
                    count: 100,
                });
            }
        }
    }

    let strategy = LinearPenalty::new(1.0);
    let config = AssignmentConfig::new(participants.clone(), history, Box::new(strategy), Some(42));

    let engine = AssignmentEngine::new();
    let result = engine.generate(config).unwrap();

    // Should still produce a valid result even with heavy history
    assert_eq!(result.assignments.len(), 4);
    for a in &result.assignments {
        assert_ne!(a.giver, a.receiver);
    }
}
