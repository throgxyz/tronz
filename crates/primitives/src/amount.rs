//! TRX amount type.
//!
//! TRON denominates value in *sun*, where `1 TRX = 1_000_000 sun`. [`Trx`]
//! wraps an `i64` sun value to match the protobuf `sint64` representation.

use core::{
    fmt,
    ops::{Add, Sub},
    str::FromStr,
};

use serde::{Deserialize, Serialize};

use crate::error::AmountError;

/// Maximum sun value that fits in TRON's signed 64-bit amount fields.
const MAX_SUN: u64 = i64::MAX as u64;

/// Number of sun in one TRX.
pub const SUN_PER_TRX: i64 = 1_000_000;

/// An amount of TRX, stored internally as `i64` sun.
///
/// User-facing constructors enforce non-negative amounts. Negative values remain
/// representable only so malformed protobuf or serialized data can be inspected
/// and round-tripped without panicking; arithmetic operations reject them.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Trx(i64);

impl Trx {
    /// Zero TRX.
    pub const ZERO: Trx = Trx(0);

    /// Construct directly from a sun value without validation.
    ///
    /// This bypasses the non-negative amount invariant. It exists for protobuf
    /// round-tripping and malformed on-chain data handling; do not use it for
    /// user input, balances, transfers, or contract calls. Prefer
    /// [`Trx::from_sun`] for raw user-facing input.
    #[doc(hidden)]
    pub const fn from_sun_unchecked(sun: i64) -> Self {
        Self(sun)
    }

    /// Construct from a raw sun value, rejecting negatives.
    pub const fn from_sun(sun: i64) -> Result<Self, AmountError> {
        if sun < 0 {
            return Err(AmountError::Negative(sun));
        }
        Ok(Self(sun))
    }

    /// The raw sun value.
    pub const fn as_sun(self) -> i64 {
        self.0
    }

    /// Checked addition. Returns `None` on `i64` overflow or if either operand
    /// is negative.
    pub fn checked_add(self, rhs: Trx) -> Option<Trx> {
        if self.0 < 0 || rhs.0 < 0 {
            return None;
        }
        self.0.checked_add(rhs.0).filter(|&v| v >= 0).map(Trx)
    }

    /// Checked subtraction. Returns `None` on `i64` overflow, if either operand
    /// is negative, or if the result would be negative.
    pub fn checked_sub(self, rhs: Trx) -> Option<Trx> {
        if self.0 < 0 || rhs.0 < 0 {
            return None;
        }
        self.0.checked_sub(rhs.0).filter(|&v| v >= 0).map(Trx)
    }
}

/// Parse a decimal TRX string (e.g. `"1.5"` or `"100"`) into sun.
///
/// The accepted syntax mirrors alloy's `parse_units` with 6 decimal places:
/// leading decimal points and `_` separators are accepted, an empty string is
/// zero, and fractional digits beyond sun precision are truncated. Negative
/// values remain invalid because native TRX amounts are non-negative.
///
/// # Examples
///
/// ```
/// use tronz_primitives::Trx;
///
/// assert_eq!("1".parse::<Trx>().unwrap().as_sun(), 1_000_000);
/// assert_eq!("1.5".parse::<Trx>().unwrap().as_sun(), 1_500_000);
/// assert_eq!(".5".parse::<Trx>().unwrap().as_sun(), 500_000);
/// assert_eq!("0.000001".parse::<Trx>().unwrap().as_sun(), 1);
/// assert_eq!("1.0000009".parse::<Trx>().unwrap().as_sun(), 1_000_000);
/// assert!("-1".parse::<Trx>().is_err());
/// ```
impl FromStr for Trx {
    type Err = AmountError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with('-') || !s.is_ascii() {
            return Err(AmountError::ParseError(s.to_owned()));
        }

        let mut normalized = s.to_owned();
        let decimal_len = if let Some(decimal_index) = normalized.find('.') {
            normalized.remove(decimal_index);
            normalized[decimal_index..].len()
        } else {
            0
        };

        // Match alloy's `parse_units`: discard fractional digits beyond the
        // selected unit precision rather than rounding or returning an error.
        if decimal_len > 6 {
            normalized.truncate(normalized.len() - (decimal_len - 6));
        }

        let mut value = 0u64;
        for byte in normalized.bytes() {
            if byte == b'_' {
                continue;
            }
            let digit = match byte {
                b'0'..=b'9' => (byte - b'0') as u64,
                _ => return Err(AmountError::ParseError(s.to_owned())),
            };
            value = value
                .checked_mul(10)
                .and_then(|v| v.checked_add(digit))
                .ok_or_else(|| AmountError::ParseError(s.to_owned()))?;
        }

        let scale = 6usize.saturating_sub(decimal_len);
        let value = value
            .checked_mul(10u64.pow(scale as u32))
            .filter(|&v| v <= MAX_SUN)
            .ok_or_else(|| AmountError::ParseError(s.to_owned()))?;
        Ok(Self(value as i64))
    }
}

/// Parse a decimal TRX string into a [`Trx`] amount.
///
/// Free-function alias for [`str::parse::<Trx>()`](Trx::from_str), mirroring
/// alloy's [`parse_ether`](https://docs.rs/alloy-primitives/latest/alloy_primitives/utils/fn.parse_ether.html)
/// for callers who prefer that style.
///
/// ```
/// use tronz_primitives::parse_trx;
///
/// assert_eq!(parse_trx("1.5").unwrap().as_sun(), 1_500_000);
/// ```
pub fn parse_trx(s: &str) -> Result<Trx, AmountError> {
    s.parse()
}

/// Format a [`Trx`] amount as a decimal string with exactly 6 fractional digits.
///
/// Free-function alias for [`Trx`]'s [`Display`](fmt::Display), mirroring alloy's
/// [`format_ether`](https://docs.rs/alloy-primitives/latest/alloy_primitives/utils/fn.format_ether.html).
///
/// ```
/// use tronz_primitives::{Trx, format_trx};
///
/// let amount = Trx::from_sun(1_500_000).unwrap();
/// assert_eq!(format_trx(amount), "1.500000");
/// ```
pub fn format_trx(amount: Trx) -> String {
    amount.to_string()
}

impl Add for Trx {
    type Output = Trx;
    /// # Panics
    ///
    /// Panics on `i64` overflow or a negative result. Use
    /// [`Trx::checked_add`] for a non-panicking alternative.
    fn add(self, rhs: Trx) -> Trx {
        self.checked_add(rhs).expect("TRX addition overflows or contains a negative operand")
    }
}

impl Sub for Trx {
    type Output = Trx;
    /// # Panics
    ///
    /// Panics on `i64` overflow or a negative result. Use
    /// [`Trx::checked_sub`] for a non-panicking alternative.
    fn sub(self, rhs: Trx) -> Trx {
        self.checked_sub(rhs)
            .expect("TRX subtraction underflows, overflows, or contains a negative operand")
    }
}

/// Formats the amount as a fixed-precision decimal TRX string, exactly (no
/// `f64`), mirroring alloy's `format_units` behavior.
///
/// ```
/// use tronz_primitives::Trx;
///
/// assert_eq!(Trx::from_sun(1_500_000).unwrap().to_string(), "1.500000");
/// assert_eq!("100".parse::<Trx>().unwrap().to_string(), "100.000000");
/// assert_eq!(Trx::from_sun(1).unwrap().to_string(), "0.000001");
/// assert_eq!("1.5".parse::<Trx>().unwrap().to_string(), "1.500000");
/// ```
impl fmt::Display for Trx {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let abs = self.0.unsigned_abs();
        let whole = abs / SUN_PER_TRX as u64;
        let frac = abs % SUN_PER_TRX as u64;
        let sign = if self.0 < 0 { "-" } else { "" };
        write!(f, "{sign}{whole}.{frac:06}")
    }
}

impl fmt::Debug for Trx {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Trx({} sun)", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sun(value: i64) -> Trx {
        Trx::from_sun(value).unwrap()
    }

    #[test]
    fn conversions() {
        assert_eq!("1".parse::<Trx>().unwrap().as_sun(), 1_000_000);
        assert_eq!("1.5".parse::<Trx>().unwrap().as_sun(), 1_500_000);
    }

    #[test]
    fn rejects_negative() {
        assert!(Trx::from_sun(-1).is_err());
        assert!("-1".parse::<Trx>().is_err());
    }

    #[test]
    fn unchecked_allows_negative() {
        assert_eq!(Trx::from_sun_unchecked(-5).as_sun(), -5);
    }

    #[test]
    fn arithmetic() {
        let a = "1".parse::<Trx>().unwrap();
        let b = "0.5".parse::<Trx>().unwrap();
        assert_eq!((a + b).as_sun(), 1_500_000);
        assert_eq!((a - b).as_sun(), 500_000);
        assert_eq!(a.checked_add(b), Some(Trx::from_sun(1_500_000).unwrap()));
    }

    #[test]
    fn parse_valid() {
        assert_eq!("1".parse::<Trx>().unwrap().as_sun(), 1_000_000);
        assert_eq!("1.5".parse::<Trx>().unwrap().as_sun(), 1_500_000);
        assert_eq!(".5".parse::<Trx>().unwrap().as_sun(), 500_000);
        assert_eq!("0.000001".parse::<Trx>().unwrap().as_sun(), 1);
        assert_eq!("100".parse::<Trx>().unwrap().as_sun(), 100_000_000);
        assert_eq!("1.000000".parse::<Trx>().unwrap().as_sun(), 1_000_000);
        assert_eq!("1_000".parse::<Trx>().unwrap().as_sun(), 1_000_000_000);
        assert_eq!("1.".parse::<Trx>().unwrap().as_sun(), 1_000_000);
        assert_eq!("".parse::<Trx>().unwrap(), Trx::ZERO);
    }

    #[test]
    fn parse_invalid() {
        assert!("-1".parse::<Trx>().is_err());
        assert!("abc".parse::<Trx>().is_err());
        assert!("1.abc".parse::<Trx>().is_err());
        assert!(" 1 ".parse::<Trx>().is_err());
        assert!("+1".parse::<Trx>().is_err());
        assert!("1.金额".parse::<Trx>().is_err());
    }

    #[test]
    fn parse_truncates_beyond_sun_precision() {
        assert_eq!("1.1234567".parse::<Trx>().unwrap().as_sun(), 1_123_456);
        assert_eq!("0.0000009".parse::<Trx>().unwrap(), Trx::ZERO);
    }

    #[test]
    fn display_is_exact() {
        assert_eq!(sun(1_500_000).to_string(), "1.500000");
        assert_eq!("100".parse::<Trx>().unwrap().to_string(), "100.000000");
        assert_eq!(sun(1).to_string(), "0.000001");
        assert_eq!(Trx::ZERO.to_string(), "0.000000");
        assert_eq!(Trx::from_sun_unchecked(-1_500_000).to_string(), "-1.500000");
    }

    #[test]
    fn display_parse_round_trip() {
        for &sun in &[0, 1, 1_000_000, 1_500_000, 100_000_000, 123_456] {
            let t = Trx::from_sun(sun).unwrap();
            assert_eq!(t.to_string().parse::<Trx>().unwrap(), t);
        }
    }

    #[test]
    fn alloy_style_helpers() {
        assert_eq!(parse_trx("1.5").unwrap().as_sun(), 1_500_000);
        assert_eq!(format_trx(sun(1_500_000)), "1.500000");
    }

    #[test]
    fn matches_alloy_unit_helpers_within_tron_range() {
        for input in ["", ".5", "1.", "1_000", "1.1234567", "9223372036854.775807"] {
            let alloy = alloy_primitives::utils::parse_units(input, 6).unwrap();
            let expected = u64::try_from(alloy).unwrap();
            assert_eq!(input.parse::<Trx>().unwrap().as_sun(), expected as i64);
        }

        for amount in [Trx::ZERO, sun(1), sun(1_500_000), "100".parse().unwrap()] {
            let alloy = alloy_primitives::utils::format_units(amount.as_sun(), 6).unwrap();
            assert_eq!(amount.to_string(), alloy);
        }
    }

    #[test]
    fn parse_accepts_max_i64_sun() {
        let max = "9223372036854.775807".parse::<Trx>().unwrap();
        assert_eq!(max.as_sun(), i64::MAX);
    }

    #[test]
    fn parse_rejects_above_max_i64_sun() {
        assert!("9223372036854.775808".parse::<Trx>().is_err());
    }

    #[test]
    fn checked_sub_rejects_negative() {
        assert!(Trx::ZERO.checked_sub(sun(1)).is_none());
    }

    #[test]
    fn checked_arithmetic_rejects_negative_operands() {
        let negative = Trx::from_sun_unchecked(-5);
        assert!(sun(10).checked_add(negative).is_none());
        assert!(negative.checked_add(sun(10)).is_none());
        assert!(sun(10).checked_sub(negative).is_none());
        assert!(negative.checked_sub(sun(10)).is_none());
    }
}
