use anyhow::{Result,bail};
use serde::Deserialize;

// A helper struct for intervals of protein weights. Inclusive on both ends: [lower, upper]

pub const WEIGHT_FACTOR: i64 = 1000000000; //as per the original implementation

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct Interval {
    pub lower: i64,
    pub upper: i64,
}

impl Interval {
    pub fn overlaps(&self, other: &Interval) -> bool {
        self.lower <= other.upper && other.lower <= self.upper
    }

    pub fn apply_weight_factor(&mut self) -> &mut Self{
        self.lower *= WEIGHT_FACTOR;
        self.upper *= WEIGHT_FACTOR;
        self
    }

    pub fn merge_with(&mut self, other: &Interval) -> &mut Self{
        self.lower = self.lower.min(other.lower);
        self.upper = self.upper.max(other.upper);
        self
    }

    pub fn clamp(&mut self, min: i64, max: i64) -> &mut Self{
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

    /// Splits one interval to n roughly equal intervals. Used for job rescheduling.
    pub fn split_to_n(&self, n: usize) -> Vec<Interval> {
        if n == 0 {
            return Vec::new();
        }

        let size = self.upper - self.lower + 1;

        // Cannot split further
        if size <= 1 || n == 1 {
            return vec![*self];
        }

        let step = (size + n as i64 - 1) / n as i64;

        let mut out = Vec::with_capacity(n);
        let mut start = self.lower;

        while start <= self.upper {
            let end = (start + step - 1).min(self.upper);

            out.push(Interval {
                lower: start,
                upper: end,
            });

            start = end + 1;
        }

        out
    }

    /// Splits interval to intervals of chunk_size. The last remaining chunk has arbitrary length < chunk size.
    /// For example: 700-800 with chunk size 70: [700-770, 771-800]. Actual chunks are multiplied with WEIGHT_FACTOR (see parameters::config)
    fn split_to_size(&self, chunk_size: i64) -> Vec<Interval> {
        let mut result = Vec::new();

        let mut start = self.lower;

        while start <= self.upper {
            let end = (start + chunk_size - 1).min(self.upper);

            result.push(Interval {
                lower: start,
                upper: end,
            });

            start = end + 1;
        }

        result
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
    fn to_chunks(self, chunk_size: i64) -> Result<Vec<Interval>>;
}

impl IntervalVecExt for Vec<Interval> {
    /// Merges all overlapping intervals, then splits each into chunks of at most `chunk_size`.
    fn to_chunks(mut self, chunk_size: i64) -> Result<Vec<Interval>> {
        if self.is_empty() {
            return Ok(vec![]);
        }

        self.sort_by_key(|i| i.lower);

        let mut result = Vec::new();
        let mut current = self[0];

        for interval in self.into_iter().skip(1) {
            if current.overlaps(&interval) {
                current.merge_with(&interval);
            } else {
                result.extend(current.split_to_size(chunk_size));
                current = interval;
            }
        }

        result.extend(current.split_to_size(chunk_size));

        Ok(result)
    }
}
