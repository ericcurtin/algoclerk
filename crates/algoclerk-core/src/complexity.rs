// SPDX-License-Identifier: Apache-2.0
//! Empirical time and space complexity classification from measurements at n, 2n, 4n, 8n.

use serde::{Deserialize, Serialize};
use std::fmt;

/// A growth class.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Class {
    /// Constant.
    Constant,
    /// Logarithmic.
    Log,
    /// Square root.
    Sqrt,
    /// Linear.
    Linear,
    /// n log n.
    NLogN,
    /// n to the 1.5.
    N15,
    /// Quadratic.
    Quadratic,
    /// Cubic.
    Cubic,
    /// Exponential.
    Exponential,
}

impl fmt::Display for Class {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Class::Constant => "O(1)",
            Class::Log => "O(log n)",
            Class::Sqrt => "O(sqrt n)",
            Class::Linear => "O(n)",
            Class::NLogN => "O(n log n)",
            Class::N15 => "O(n^1.5)",
            Class::Quadratic => "O(n^2)",
            Class::Cubic => "O(n^3)",
            Class::Exponential => "O(2^n)",
        })
    }
}

/// Classify a growth ratio per doubling of the input.
pub fn class_of_ratio(r: f64) -> Class {
    match r {
        r if r < 1.08 => Class::Constant,
        r if r < 1.3 => Class::Log,
        r if r < 1.7 => Class::Sqrt,
        r if r < 2.12 => Class::Linear,
        r if r < 2.65 => Class::NLogN,
        r if r < 3.4 => Class::N15,
        r if r < 5.5 => Class::Quadratic,
        r if r < 12.0 => Class::Cubic,
        _ => Class::Exponential,
    }
}

/// Classify a series measured at doubling sizes.
pub fn classify(samples: &[f64]) -> Class {
    if samples.len() < 2 {
        return Class::Constant;
    }
    let first = samples[0].max(1.0);
    let last = samples[samples.len() - 1].max(1.0);
    // Mostly flat (a tiny absolute drift is allowed for constants).
    if last - first <= 8.0 && last <= first * 1.1 + 8.0 {
        return Class::Constant;
    }
    // Geometric mean of the later ratios (the first one is dominated by constant offsets).
    let skip = if samples.len() > 3 { 1 } else { 0 };
    let ratios: Vec<f64> = samples.windows(2).skip(skip).map(|w| w[1].max(1.0) / w[0].max(1.0)).collect();
    let g = (ratios.iter().map(|r| r.ln()).sum::<f64>() / ratios.len() as f64).exp();
    class_of_ratio(g)
}

/// Measured behaviour of a solution.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Complexity {
    /// Time class.
    pub time: Class,
    /// Auxiliary space class.
    pub space: Class,
    /// `(n, cost, auxiliary cells)` per measured size.
    pub samples: Vec<(i64, u64, u64)>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn series(f: impl Fn(f64) -> f64) -> Vec<f64> {
        (0..4).map(|i| f(64.0 * f64::from(1 << i))).collect()
    }

    #[test]
    fn classifies_textbook_shapes() {
        assert_eq!(classify(&series(|_| 10.0)), Class::Constant);
        assert_eq!(classify(&series(|n| 6.0 * n.log2() + 3.0)), Class::Log);
        assert_eq!(classify(&series(|n| 7.0 * n + 40.0)), Class::Linear);
        assert_eq!(classify(&series(|n| 3.0 * n * n.log2() + 20.0)), Class::NLogN);
        assert_eq!(classify(&series(|n| 2.0 * n * n + 5.0 * n)), Class::Quadratic);
        assert_eq!(classify(&series(|n| n * n * n / 100.0)), Class::Cubic);
        assert_eq!(classify(&series(|n| 4.0 * n.sqrt() + 5.0)), Class::Sqrt);
    }
}
