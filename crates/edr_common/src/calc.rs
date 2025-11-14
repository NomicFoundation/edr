//! Commonly used calculations.

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
    if len.is_multiple_of(2) {
        let before_mid = values.get(mid - 1).expect("values is not empty");
        let mid = values.get(mid).expect("values is not empty");
        (before_mid + mid) / 2
    } else {
        *values.get(mid).expect("values is not empty")
    }
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
}
