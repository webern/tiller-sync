//! Amount type for handling monetary values with optional dollar signs.
//!
//! This module provides the `Amount` type which wraps `Decimal` and handles
//! parsing values that may or may not include a dollar and commas.

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::error::Error;
use std::fmt;
use std::fmt::{Debug, Display, Formatter};
use std::str::FromStr;

/// Represents how dollar amounts were (or should be) formatted.
///
/// # Examples
///  - `AmountFormat{ dollar: true, commas: true }` -> `-$60,000.00`
///  - `AmountFormat{ dollar: false, commas: true }` -> `-60,000.00`
///  - `AmountFormat{ dollar: false, commas: false }` -> `-60000.00`
///  - `AmountFormat{ dollar: true, commas: false }` -> `-$60000.00`
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AmountFormat {
    /// Whether a dollar sign is present in the formatting.
    dollar: bool,
    /// Whether commas are present as thousands separators in the formatting.
    commas: bool,
}

impl Default for AmountFormat {
    fn default() -> Self {
        DEFAULT_FORMAT
    }
}

/// The default format has a dollar sign and commas: e.g. `-$60,000.00`.
const DEFAULT_FORMAT: AmountFormat = AmountFormat {
    dollar: true,
    commas: true,
};

/// Represents a dollar amount.
///
/// This type wraps `Decimal` and provides custom serialization/deserialization
/// to handle amounts that may be formatted with or without dollar signs or commas.
///
/// Formatting is considered significant for the purposes of equality, so for numeric comparisons,
/// you should access the `Decimal` value and use that.
///
/// # Examples
///
/// Parsing with dollar sign:
/// ```
/// # use tiller_sync::model::Amount;
/// # use std::str::FromStr;
/// let amount = Amount::from_str("-$50.00").unwrap();
/// assert_eq!(amount.to_string(), "-$50.00");
/// ```
///
/// Parsing without dollar sign:
/// ```
/// # use tiller_sync::model::Amount;
/// # use std::str::FromStr;
/// let amount = Amount::from_str("-50.00").unwrap();
/// assert_ne!(amount.to_string(), "-$50.00");
/// assert_eq!(amount.to_string(), "-50.00");
/// ```
///
/// Value equivalency, but not absolute equivalency
/// ```
/// # use tiller_sync::model::Amount;
/// # use std::str::FromStr;
/// let a = Amount::from_str("-5000.00").unwrap();
/// let b = Amount::from_str("-$5,000.00").unwrap();
/// assert_ne!(a, b);
/// assert_ne!(a.to_string(), b.to_string());
/// assert_eq!(a.value(), b.value());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Amount {
    /// The parsed numerical value.
    value: Decimal,
    /// The way the numerical value was parsed from, or should be written to, a `String`.
    format: AmountFormat,
}

impl Amount {
    /// Creates a new Amount from a Decimal value with default `String` formatting.
    pub const fn new(value: Decimal) -> Self {
        Self {
            value,
            format: DEFAULT_FORMAT,
        }
    }

    /// Creates a new Amount from a Decimal value with default specified formatting.
    pub const fn new_with_format(value: Decimal, format: AmountFormat) -> Self {
        Self { value, format }
    }

    /// Returns the underlying Decimal value.
    pub fn value(&self) -> Decimal {
        self.value
    }

    /// Returns true if the amount is zero.
    pub fn is_zero(&self) -> bool {
        self.value().is_zero()
    }

    /// Returns true if the amount is positive.
    pub fn is_positive(&self) -> bool {
        !self.is_zero() && self.value().is_sign_positive()
    }

    /// Returns true if the amount is negative.
    pub fn is_negative(&self) -> bool {
        self.value().is_sign_negative()
    }
}

/// An error that can occur when parsing strings into `Decimal` values.
pub struct AmountError(rust_decimal::Error);

impl Debug for AmountError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

impl Display for AmountError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl std::error::Error for AmountError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.0)
    }
}

impl FromStr for Amount {
    type Err = AmountError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut dollar_sign = false;

        // Remove whitespace
        let trimmed = s.trim();

        // Handle empty string
        if trimmed.is_empty() {
            return Ok(Amount::default());
        }

        // Remove dollar sign if present
        let without_dollar = if let Some(after_minus) = trimmed.strip_prefix('-') {
            // Negative number: could be "-$50.00" or "-50.00"
            if let Some(after_dollar) = after_minus.strip_prefix('$') {
                dollar_sign = true;
                format!("-{after_dollar}")
            } else {
                trimmed.to_string()
            }
        } else if let Some(after_dollar) = trimmed.strip_prefix('$') {
            // Positive number with dollar sign: "$50.00"
            dollar_sign = true;
            after_dollar.to_string()
        } else {
            // No dollar sign
            trimmed.to_string()
        };

        // Remove commas (thousand separators)
        let without_commas = without_dollar.replace(',', "");
        let commas = without_commas.len() < without_dollar.len();

        // Parse the decimal value
        let value = Decimal::from_str(&without_commas).map_err(AmountError)?;
        Ok(Amount {
            value,
            format: AmountFormat {
                dollar: dollar_sign,
                commas,
            },
        })
    }
}

impl fmt::Display for Amount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (sign, num) = if self.is_negative() {
            (String::from("-"), self.value().abs())
        } else {
            (String::new(), self.value())
        };

        let dol = if self.format.dollar {
            String::from("$")
        } else {
            String::new()
        };

        if self.format.commas {
            write!(
                f,
                "{sign}{dol}{}",
                format_num::format_num!(",.2", num.to_f64().unwrap_or_default())
            )
        } else {
            write!(f, "{sign}{dol}{num}")
        }
    }
}

impl Serialize for Amount {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Serialize as a string with dollar sign
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Amount {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Amount::from_str(&s).map_err(serde::de::Error::custom)
    }
}

impl From<Decimal> for Amount {
    fn from(value: Decimal) -> Self {
        Amount::new(value)
    }
}

impl From<Amount> for Decimal {
    fn from(amount: Amount) -> Self {
        amount.value()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_with_dollar_sign() {
        let amount = Amount::from_str("$50.00").unwrap();
        assert_eq!(amount.value(), Decimal::from_str("50.00").unwrap());
    }

    #[test]
    fn test_parse_without_dollar_sign() {
        let amount = Amount::from_str("50.00").unwrap();
        assert_eq!(amount.value(), Decimal::from_str("50.00").unwrap());
    }

    #[test]
    fn test_parse_negative_with_dollar_sign() {
        let amount = Amount::from_str("-$50.00").unwrap();
        assert_eq!(amount.value(), Decimal::from_str("-50.00").unwrap());
    }

    #[test]
    fn test_parse_negative_without_dollar_sign() {
        let amount = Amount::from_str("-50.00").unwrap();
        assert_eq!(amount.value(), Decimal::from_str("-50.00").unwrap());
    }

    #[test]
    fn test_parse_empty_string() {
        let amount = Amount::from_str("").unwrap();
        assert_eq!(amount.value(), Decimal::ZERO);
    }

    #[test]
    fn test_parse_whitespace() {
        let amount = Amount::from_str("  $50.00  ").unwrap();
        assert_eq!(amount.value(), Decimal::from_str("50.00").unwrap());
    }

    #[test]
    fn test_display_positive() {
        let amount = Amount::new(Decimal::from_str("50.00").unwrap());
        assert_eq!(amount.to_string(), "$50.00");
    }

    #[test]
    fn test_display_negative() {
        let amount = Amount::new(Decimal::from_str("-50.00").unwrap());
        assert_eq!(amount.to_string(), "-$50.00");
    }

    #[test]
    fn test_display_zero() {
        let amount = Amount::new(Decimal::ZERO);
        assert_eq!(amount.to_string(), "$0.00");
    }

    #[test]
    fn test_serialize() {
        let amount = Amount::new(Decimal::from_str("50.00").unwrap());
        let json = serde_json::to_string(&amount).unwrap();
        assert_eq!(json, "\"$50.00\"");
    }

    #[test]
    fn test_deserialize_with_dollar() {
        let json = "\"$50.00\"";
        let amount: Amount = serde_json::from_str(json).unwrap();
        assert_eq!(amount.value(), Decimal::from_str("50.00").unwrap());
    }

    #[test]
    fn test_deserialize_without_dollar() {
        let json = "\"50.00\"";
        let amount: Amount = serde_json::from_str(json).unwrap();
        assert_eq!(amount.value(), Decimal::from_str("50.00").unwrap());
    }

    #[test]
    fn test_deserialize_negative() {
        let json = "\"-$50.00\"";
        let amount: Amount = serde_json::from_str(json).unwrap();
        assert_eq!(amount.value(), Decimal::from_str("-50.00").unwrap());
    }

    #[test]
    fn test_equality() {
        let a1 = Amount::from_str("$50.00").unwrap();
        let a2 = Amount::from_str("50.00").unwrap();
        assert_ne!(a1, a2);
        assert_eq!(a1.value(), a2.value());
    }

    #[test]
    fn test_ordering() {
        let a1 = Amount::from_str("$30.00").unwrap();
        let a2 = Amount::from_str("$50.00").unwrap();
        assert!(a1 < a2);
    }

    #[test]
    fn test_is_zero() {
        let zero = Amount::from_str("$0.00").unwrap();
        assert!(zero.is_zero());

        let non_zero = Amount::from_str("$50.00").unwrap();
        assert!(!non_zero.is_zero());
    }

    #[test]
    fn test_zero_is_not_positive_or_negative() {
        let zero = Amount::from_str("$0.00").unwrap();
        assert!(!zero.is_positive());
        assert!(!zero.is_negative());
        assert!(zero.is_zero());
    }

    #[test]
    fn test_is_positive() {
        let positive = Amount::from_str("$50.00").unwrap();
        assert!(positive.is_positive());

        let negative = Amount::from_str("-$50.00").unwrap();
        assert!(!negative.is_positive());
    }

    #[test]
    fn test_is_negative() {
        let negative = Amount::from_str("-$50.00").unwrap();
        assert!(negative.is_negative());

        let positive = Amount::from_str("$50.00").unwrap();
        assert!(!positive.is_negative());
    }

    #[test]
    fn test_parse_with_commas() {
        let amount = Amount::from_str("$1,000.00").unwrap();
        assert_eq!(amount.value(), Decimal::from_str("1000.00").unwrap());
    }

    #[test]
    fn test_parse_large_amount_with_commas() {
        let amount = Amount::from_str("-$60,000.00").unwrap();
        assert_eq!(amount.value(), Decimal::from_str("-60000.00").unwrap());
    }

    #[test]
    fn test_parse_multiple_commas() {
        let amount = Amount::from_str("$1,234,567.89").unwrap();
        assert_eq!(amount.value(), Decimal::from_str("1234567.89").unwrap());
    }

    #[test]
    fn test_parse_commas_without_dollar() {
        let amount = Amount::from_str("1,000.00").unwrap();
        assert_eq!(amount.value(), Decimal::from_str("1000.00").unwrap());
    }

    #[test]
    fn test_parse_retain_commas_no_dollarsign() {
        let s = "1,000,000.00";
        let amount = Amount::from_str(s).unwrap();
        let actual = amount.to_string();
        assert_eq!(actual, s);
    }

    #[test]
    fn test_parse_no_commas_retain_dollar_sign() {
        let s = "-$1000000.00";
        let amount = Amount::from_str(s).unwrap();
        let actual = amount.to_string();
        assert_eq!(actual, s);
    }
}
