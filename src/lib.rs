//! # Weighted Random Assignment Engine
//!
//! A generic weighted random assignment engine with historical penalty support.
//!
//! This library provides a configurable system for generating random assignments
//! (pairings) between participants while minimizing repeated pairings across
//! multiple runs through penalty weights.
//!
//! ## Use Cases
//!
//! - Fair task assignment
//! - Matching participants over multiple rounds
//! - Scheduling with historical fairness
//!
//! ## Example
//!
//! ```
//! use weighted_random_assignment::core::models::*;
//! use weighted_random_assignment::core::penalty::{PenaltyStrategy, LinearPenalty};
//! use weighted_random_assignment::engine::AssignmentEngine;
//!
//! let participants = vec!["Alice".to_string(), "Bob".to_string(), "Charlie".to_string()];
//! let history = vec![];
//! let strategy = LinearPenalty::new(1.0);
//! let config = AssignmentConfig::new(participants, history, Box::new(strategy), None);
//! let engine = AssignmentEngine::new();
//! let result = engine.generate(config).unwrap();
//! ```

pub mod core;
pub mod engine;
pub mod infra;
