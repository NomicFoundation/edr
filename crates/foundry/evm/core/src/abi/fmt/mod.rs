//! Helpers for formatting Ethereum types.

use alloy_primitives::{Sign, I256, U256};
use edr_common::calc::to_exp_notation;
use yansi::Paint;

mod console;
pub use console::{console_format, ConsoleFmt, FormatSpec};

mod dynamic;
pub use dynamic::{format_token, format_token_raw, format_tokens, format_tokens_raw, parse_tokens};

mod transactions;
mod ui;

pub use ui::{get_pretty_block_attr, get_pretty_tx_attr, UIfmt};

/// Formats a U256 number to string, adding an exponential notation _hint_ if it
/// is larger than `10_000`, with a precision of `4` figures, and trimming the
/// trailing zeros.
///
/// # Examples
///
/// ```
/// use alloy_primitives::U256;
/// use foundry_evm_core::abi::fmt::format_uint_exp as f;
///
/// # yansi::disable();
/// assert_eq!(f(U256::from(0)), "0");
/// assert_eq!(f(U256::from(1234)), "1234");
/// assert_eq!(f(U256::from(1234567890)), "1234567890 [1.234e9]");
/// assert_eq!(f(U256::from(1000000000000000000_u128)), "1000000000000000000 [1e18]");
/// assert_eq!(f(U256::from(10000000000000000000000_u128)), "10000000000000000000000 [1e22]");
/// ```
pub fn format_uint_exp(num: U256) -> String {
    if num < U256::from(10_000) {
        return num.to_string();
    }

    let exp = to_exp_notation(num, 4, true, Sign::Positive);
    format!("{num} {}", format!("[{exp}]").dim())
}

/// Formats a U256 number to string, adding an exponential notation _hint_.
///
/// Same as [`format_uint_exp`].
///
/// # Examples
///
/// ```
/// use alloy_primitives::I256;
/// use foundry_evm_core::abi::fmt::format_int_exp as f;
///
/// # yansi::disable();
/// assert_eq!(f(I256::try_from(0).unwrap()), "0");
/// assert_eq!(f(I256::try_from(-1).unwrap()), "-1");
/// assert_eq!(f(I256::try_from(1234).unwrap()), "1234");
/// assert_eq!(f(I256::try_from(1234567890).unwrap()), "1234567890 [1.234e9]");
/// assert_eq!(f(I256::try_from(-1234567890).unwrap()), "-1234567890 [-1.234e9]");
/// assert_eq!(f(I256::try_from(1000000000000000000_u128).unwrap()), "1000000000000000000 [1e18]");
/// assert_eq!(
///     f(I256::try_from(10000000000000000000000_u128).unwrap()),
///     "10000000000000000000000 [1e22]"
/// );
/// assert_eq!(
///     f(I256::try_from(-10000000000000000000000_i128).unwrap()),
///     "-10000000000000000000000 [-1e22]"
/// );
/// ```
pub fn format_int_exp(num: I256) -> String {
    let (sign, abs) = num.into_sign_and_abs();
    if abs < U256::from(10_000) {
        return format!("{sign}{abs}");
    }

    let exp = to_exp_notation(abs, 4, true, sign);
    format!("{sign}{abs} {}", format!("[{exp}]").dim())
}
