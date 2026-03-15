//! Penalty strategies for adjusting assignment weights based on historical pairings.
//!
//! Penalty strategies determine how the base weight of a potential pairing is
//! modified based on how many times that pairing has occurred in the past.

use std::fmt::Debug;

/// Trait for penalty strategies that adjust assignment weights.
///
/// Implementors of this trait define how historical pairing counts affect the
/// probability of a pairing being selected in future runs.
pub trait PenaltyStrategy: Send + Sync + Debug {
    /// Calculates the adjusted weight for a potential pairing.
    ///
    /// # Arguments
    ///
    /// * `base_weight` - The original weight before penalty adjustment.
    /// * `historical_count` - The number of times this pairing has occurred previously.
    ///
    /// # Returns
    ///
    /// The adjusted weight, which must be >= 0.0.
    fn adjusted_weight(&self, base_weight: f64, historical_count: u32) -> f64;

    /// Returns the name of this penalty strategy.
    fn name(&self) -> &str;
}

/// Linear penalty strategy.
///
/// Formula: `adjusted_weight = base_weight / (1 + penalty_factor * historical_count)`
///
/// This is the default strategy. It applies a proportional reduction based on
/// historical frequency. Higher penalty factors cause stronger avoidance of
/// repeated pairings.
#[derive(Debug, Clone)]
pub struct LinearPenalty {
    /// The penalty factor applied per historical occurrence.
    pub penalty_factor: f64,
}

impl LinearPenalty {
    /// Creates a new linear penalty strategy.
    ///
    /// # Arguments
    ///
    /// * `penalty_factor` - The factor applied per historical occurrence. Must be >= 0.0 and finite.
    ///
    /// # Panics
    ///
    /// Panics if `penalty_factor` is negative, NaN, or infinite.
    pub fn new(penalty_factor: f64) -> Self {
        assert!(
            penalty_factor >= 0.0 && penalty_factor.is_finite(),
            "penalty_factor must be non-negative and finite, got {}",
            penalty_factor
        );
        Self { penalty_factor }
    }
}

impl PenaltyStrategy for LinearPenalty {
    fn adjusted_weight(&self, base_weight: f64, historical_count: u32) -> f64 {
        let weight = base_weight / (1.0 + self.penalty_factor * f64::from(historical_count));
        weight.max(0.0)
    }

    fn name(&self) -> &str {
        "linear"
    }
}

/// Exponential penalty strategy.
///
/// Formula: `adjusted_weight = base_weight * decay_rate^historical_count`
///
/// This strategy applies exponentially increasing penalties for repeated
/// pairings. Useful when even a single repeat should be strongly discouraged.
#[derive(Debug, Clone)]
pub struct ExponentialPenalty {
    /// The decay rate applied exponentially. Should be between 0.0 and 1.0.
    pub decay_rate: f64,
}

impl ExponentialPenalty {
    /// Creates a new exponential penalty strategy.
    ///
    /// # Arguments
    ///
    /// * `decay_rate` - The decay rate per occurrence. Must be in [0.0, 1.0].
    ///
    /// # Panics
    ///
    /// Panics if `decay_rate` is outside the range [0.0, 1.0] or is NaN.
    pub fn new(decay_rate: f64) -> Self {
        assert!(
            (0.0..=1.0).contains(&decay_rate),
            "decay_rate must be between 0.0 and 1.0, got {}",
            decay_rate
        );
        Self { decay_rate }
    }
}

impl PenaltyStrategy for ExponentialPenalty {
    fn adjusted_weight(&self, base_weight: f64, historical_count: u32) -> f64 {
        let weight = base_weight * self.decay_rate.powf(f64::from(historical_count));
        weight.max(0.0)
    }

    fn name(&self) -> &str {
        "exponential"
    }
}

/// Threshold penalty strategy.
///
/// Assigns a reduced weight once the historical count exceeds a threshold.
/// Below the threshold, the base weight is used unchanged.
#[derive(Debug, Clone)]
pub struct ThresholdPenalty {
    /// The number of occurrences before penalty kicks in.
    pub threshold: u32,
    /// The weight multiplier applied once threshold is exceeded (0.0 to 1.0).
    pub reduced_weight_factor: f64,
}

impl ThresholdPenalty {
    /// Creates a new threshold penalty strategy.
    ///
    /// # Arguments
    ///
    /// * `threshold` - The historical count at which the penalty activates.
    /// * `reduced_weight_factor` - The multiplier applied when threshold is exceeded. Must be in [0.0, 1.0].
    ///
    /// # Panics
    ///
    /// Panics if `reduced_weight_factor` is outside the range [0.0, 1.0] or is NaN.
    pub fn new(threshold: u32, reduced_weight_factor: f64) -> Self {
        assert!(
            (0.0..=1.0).contains(&reduced_weight_factor),
            "reduced_weight_factor must be between 0.0 and 1.0, got {}",
            reduced_weight_factor
        );
        Self {
            threshold,
            reduced_weight_factor,
        }
    }
}

impl PenaltyStrategy for ThresholdPenalty {
    fn adjusted_weight(&self, base_weight: f64, historical_count: u32) -> f64 {
        if historical_count >= self.threshold {
            (base_weight * self.reduced_weight_factor).max(0.0)
        } else {
            base_weight
        }
    }

    fn name(&self) -> &str {
        "threshold"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_penalty_no_history() {
        let strategy = LinearPenalty::new(1.0);
        assert!((strategy.adjusted_weight(1.0, 0) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_linear_penalty_with_history() {
        let strategy = LinearPenalty::new(1.0);
        // 1.0 / (1 + 1.0 * 2) = 1/3
        let weight = strategy.adjusted_weight(1.0, 2);
        assert!((weight - 1.0 / 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_linear_penalty_high_factor() {
        let strategy = LinearPenalty::new(10.0);
        // 1.0 / (1 + 10.0 * 1) = 1/11
        let weight = strategy.adjusted_weight(1.0, 1);
        assert!((weight - 1.0 / 11.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_linear_penalty_zero_factor() {
        let strategy = LinearPenalty::new(0.0);
        // No penalty applied
        assert!((strategy.adjusted_weight(1.0, 5) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_exponential_penalty_no_history() {
        let strategy = ExponentialPenalty::new(0.5);
        assert!((strategy.adjusted_weight(1.0, 0) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_exponential_penalty_with_history() {
        let strategy = ExponentialPenalty::new(0.5);
        // 1.0 * 0.5^2 = 0.25
        let weight = strategy.adjusted_weight(1.0, 2);
        assert!((weight - 0.25).abs() < f64::EPSILON);
    }

    #[test]
    fn test_threshold_penalty_below_threshold() {
        let strategy = ThresholdPenalty::new(3, 0.1);
        assert!((strategy.adjusted_weight(1.0, 2) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_threshold_penalty_at_threshold() {
        let strategy = ThresholdPenalty::new(3, 0.1);
        let weight = strategy.adjusted_weight(1.0, 3);
        assert!((weight - 0.1).abs() < f64::EPSILON);
    }

    #[test]
    fn test_threshold_penalty_above_threshold() {
        let strategy = ThresholdPenalty::new(3, 0.1);
        let weight = strategy.adjusted_weight(1.0, 5);
        assert!((weight - 0.1).abs() < f64::EPSILON);
    }

    #[test]
    fn test_non_negative_weights() {
        let strategy = LinearPenalty::new(1000.0);
        let weight = strategy.adjusted_weight(1.0, 1000);
        assert!(weight >= 0.0);
    }

    #[test]
    #[should_panic(expected = "penalty_factor must be non-negative")]
    fn test_linear_penalty_rejects_negative() {
        LinearPenalty::new(-1.0);
    }

    #[test]
    #[should_panic(expected = "penalty_factor must be non-negative and finite")]
    fn test_linear_penalty_rejects_nan() {
        LinearPenalty::new(f64::NAN);
    }

    #[test]
    #[should_panic(expected = "penalty_factor must be non-negative and finite")]
    fn test_linear_penalty_rejects_infinity() {
        LinearPenalty::new(f64::INFINITY);
    }

    #[test]
    #[should_panic(expected = "decay_rate must be between 0.0 and 1.0")]
    fn test_exponential_penalty_rejects_greater_than_one() {
        ExponentialPenalty::new(1.5);
    }

    #[test]
    #[should_panic(expected = "decay_rate must be between 0.0 and 1.0")]
    fn test_exponential_penalty_rejects_negative() {
        ExponentialPenalty::new(-0.1);
    }

    #[test]
    #[should_panic(expected = "reduced_weight_factor must be between 0.0 and 1.0")]
    fn test_threshold_penalty_rejects_negative_factor() {
        ThresholdPenalty::new(2, -0.5);
    }

    #[test]
    #[should_panic(expected = "reduced_weight_factor must be between 0.0 and 1.0")]
    fn test_threshold_penalty_rejects_factor_greater_than_one() {
        ThresholdPenalty::new(2, 1.5);
    }
}
