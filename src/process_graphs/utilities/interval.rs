use anyhow::{Result, bail};
use serde::Deserialize;

pub const WEIGHT_FACTOR: i64 = 1000000000;

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct Interval {
    pub lower: i64,
    pub upper: i64,
}

impl Interval {
    pub fn overlaps(&self, other: &Interval) -> bool {
        self.lower <= other.upper && other.lower <= self.upper
    }

    pub fn merge_with(&mut self, other: &Interval) -> &mut Self {
        self.lower = self.lower.min(other.lower);
        self.upper = self.upper.max(other.upper);
        self
    }

    pub fn clamp(&mut self, min: i64, max: i64) -> &mut Self {
        self.lower = self.lower.clamp(min, max);
        self.upper = self.upper.clamp(min, max);
        self
    }

    pub fn validate(&self) -> Result<&Self> {
        if self.lower > self.upper {
            bail!(
                "Invalid interval: lower ({}) > upper ({})",
                self.lower,
                self.upper
            );
        }
        Ok(self)
    }
}

impl std::fmt::Display for Interval {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}..{}",
            self.lower as f64 / WEIGHT_FACTOR as f64,
            self.upper as f64 / WEIGHT_FACTOR as f64
        )
    }
}

pub trait IntervalVecExt {
    fn merge_and_sort(self) -> Result<Box<[Interval]>>;
}

impl IntervalVecExt for Vec<Interval> {
    /// Sorts intervals and merges all overlapping intervals.
    fn merge_and_sort(mut self) -> Result<Box<[Interval]>> {
        if self.is_empty() {
            return Ok(Box::new([]));
        }

        self.sort_by_key(|i| i.lower);

        let mut result = Vec::new();
        let mut current = self[0];

        for interval in self.into_iter().skip(1) {
            if current.overlaps(&interval) {
                current.merge_with(&interval);
            } else {
                result.push(current);
                current = interval;
            }
        }

        result.push(current);

        Ok(result.into_boxed_slice())
    }
}
