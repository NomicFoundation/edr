//! Commonly used calculations.

use alloy_primitives::{Sign, U256};

/// Returns the mean of the slice.
#[inline]
pub fn mean(values: &[u64]) -> u64 {
    if values.is_empty() {
        return 0;
    }

    (values.iter().map(|x| u128::from(*x)).sum::<u128>() / values.len() as u128) as u64
}

/// Returns the median of a _sorted_ slice.
#[inline]
pub fn median_sorted(values: &[u64]) -> u64 {
    if values.is_empty() {
        return 0;
    }

    let len = values.len();
    let mid = len / 2;
    if len % 2 == 0 {
        (values[mid - 1] + values[mid]) / 2
    } else {
        values[mid]
    }
}

/// Returns the number expressed as a string in exponential notation
/// with the given precision (number of significant figures),
/// optionally removing trailing zeros from the mantissa.
///
/// Examples:
///
/// ```text
/// precision = 4, trim_end_zeroes = false
///     1234124124 -> 1.234e9
///     10000000 -> 1.000e7
/// precision = 3, trim_end_zeroes = true
///     1234124124 -> 1.23e9
///     10000000 -> 1e7
/// ```
#[inline]
pub fn to_exp_notation(value: U256, precision: usize, trim_end_zeros: bool, sign: Sign) -> String {
    let stringified = value.to_string();
    let exponent = stringified.len() - 1;
    let mut mantissa = stringified.chars().take(precision).collect::<String>();

    // optionally remove trailing zeros
    if trim_end_zeros {
        mantissa = mantissa.trim_end_matches('0').to_string();
    }

    // Place a decimal point only if needed
    // e.g. 1234 -> 1.234e3 (needed)
    //      5 -> 5 (not needed)
    if mantissa.len() > 1 {
        mantissa.insert(1, '.');
    }

    format!("{sign}{mantissa}e{exponent}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calc_mean_empty() {
        let m = mean(&[]);
        assert_eq!(m, 0);
    }

    #[test]
    fn calc_mean() {
        let m = mean(&[0, 1, 2, 3, 4, 5, 6]);
        assert_eq!(m, 3);
    }

    #[test]
    fn calc_mean_overflow() {
        let m = mean(&[
            0,
            1,
            2,
            u64::from(u32::MAX),
            3,
            u64::from(u16::MAX),
            u64::MAX,
            6,
        ]);
        assert_eq!(m, 2305843009750573057);
    }

    #[test]
    fn calc_median_empty() {
        let m = median_sorted(&[]);
        assert_eq!(m, 0);
    }

    #[test]
    fn calc_median() {
        let mut values = vec![29, 30, 31, 40, 59, 61, 71];
        values.sort();
        let m = median_sorted(&values);
        assert_eq!(m, 40);
    }

    #[test]
    fn calc_median_even() {
        let mut values = vec![80, 90, 30, 40, 50, 60, 10, 20];
        values.sort();
        let m = median_sorted(&values);
        assert_eq!(m, 45);
    }

    #[test]
    fn test_format_to_exponential_notation() {
        let value = 1234124124u64;

        let formatted = to_exp_notation(U256::from(value), 4, false, Sign::Positive);
        assert_eq!(formatted, "1.234e9");

        let formatted = to_exp_notation(U256::from(value), 3, true, Sign::Positive);
        assert_eq!(formatted, "1.23e9");

        let value = 10000000u64;

        let formatted = to_exp_notation(U256::from(value), 4, false, Sign::Positive);
        assert_eq!(formatted, "1.000e7");

        let formatted = to_exp_notation(U256::from(value), 3, true, Sign::Positive);
        assert_eq!(formatted, "1e7");
    }
}
