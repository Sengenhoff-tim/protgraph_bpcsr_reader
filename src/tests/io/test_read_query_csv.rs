use std::io::Write;

use rand::Rng;
use rand::RngExt;
use tempfile::NamedTempFile;

use crate::parameters::io::read_query_csv::read_query_csv;
use crate::process_graphs::utilities::WEIGHT_FACTOR;

const N: usize = 200;
const LOWER_BOUND: i64 = 10;
const UPPER_BOUND: i64 = 10000;

/// Generates a random f64 in [min, max], rounded to WEIGHT_FACTOR precision
/// so that (value * WEIGHT_FACTOR).round() as i64 is exact / lossless.
fn random_value(rng: &mut impl Rng, min: f64, max: f64) -> f64 {
    let raw = rng.random_range(min..=max);
    (raw * WEIGHT_FACTOR as f64).round() / WEIGHT_FACTOR as f64
}

/// Writes N random rows to a temp CSV and returns the file plus
/// the expected scaled (lower, upper) pairs, computed independently
/// of read_query_csv's internal logic.
fn write_random_csv(n: usize, min: f64, max: f64) -> (NamedTempFile, Vec<(i64, i64)>) {
    let mut rng = rand::rng();
    let mut file = NamedTempFile::new().expect("failed to create temp file");
    let mut expected = Vec::with_capacity(n);

    writeln!(file, "lower,upper").unwrap();

    for _ in 0..n {
        let a = random_value(&mut rng, min, max);
        let b = random_value(&mut rng, min, max);
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };

        writeln!(file, "{},{}", lo, hi).unwrap();

        let exp_lower = (lo * WEIGHT_FACTOR as f64).round() as i64;
        let exp_upper = (hi * WEIGHT_FACTOR as f64).round() as i64;
        expected.push((exp_lower, exp_upper));
    }

    file.flush().unwrap();
    (file, expected)
}

#[test]
fn test_read_query_csv_random_intervals() {
    let (file, expected) = write_random_csv(N, LOWER_BOUND as f64, UPPER_BOUND as f64);
    let path = file.path().to_path_buf();

    let result = read_query_csv(&path, LOWER_BOUND, UPPER_BOUND);
    assert!(result.is_ok(), "read_query_csv failed: {:?}", result.err());

    let intervals = result.unwrap();
    assert_eq!(intervals.len(), N);

    let scaled_lower_bound = LOWER_BOUND * WEIGHT_FACTOR;
    let scaled_upper_bound = UPPER_BOUND * WEIGHT_FACTOR;

    for (interval, (exp_lower, exp_upper)) in intervals.iter().zip(expected.iter()) {
        // Exact match against independently computed expected values
        assert_eq!(interval.lower, *exp_lower);
        assert_eq!(interval.upper, *exp_upper);

        // Invariants: valid ordering and within clamped bounds
        assert!(interval.lower <= interval.upper);
        assert!(interval.lower >= scaled_lower_bound);
        assert!(interval.upper <= scaled_upper_bound);
    }
}

#[test]
fn test_read_query_csv_clamps_out_of_range_values() {
    let mut file = NamedTempFile::new().expect("failed to create temp file");
    writeln!(file, "lower,upper").unwrap();
    // Below lower bound and above upper bound, to exercise clamp()
    writeln!(file, "1,5").unwrap();
    writeln!(file, "9990,20000").unwrap();
    file.flush().unwrap();

    let path = file.path().to_path_buf();
    let intervals = read_query_csv(&path, LOWER_BOUND, UPPER_BOUND).unwrap();

    let scaled_lower_bound = LOWER_BOUND * WEIGHT_FACTOR;
    let scaled_upper_bound = UPPER_BOUND * WEIGHT_FACTOR;

    assert_eq!(intervals[0].lower, scaled_lower_bound);
    assert_eq!(intervals[1].upper, scaled_upper_bound);
}

#[test]
#[should_panic]
fn test_read_query_csv_panics_on_inverted_bounds() {
    // Interval::clamp -> i64::clamp panics if min > max (assert!(min <= max) in std).
    // Passing lower_bound > upper_bound should surface that panic.
    let mut file = NamedTempFile::new().expect("failed to create temp file");
    writeln!(file, "lower,upper").unwrap();
    writeln!(file, "50,60").unwrap();
    file.flush().unwrap();

    let path = file.path().to_path_buf();
    let _ = read_query_csv(&path, UPPER_BOUND, LOWER_BOUND); // inverted on purpose
}