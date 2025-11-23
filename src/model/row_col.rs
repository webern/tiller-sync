use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

/// A coordinate representing a (row, column) position.
/// Serializes to a string format like "(0, 1)" for JSON compatibility.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct RowCol(pub usize, pub usize);

impl RowCol {
    pub fn new(row: usize, col: usize) -> Self {
        Self(row, col)
    }
}

impl fmt::Display for RowCol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {})", self.0, self.1)
    }
}

impl FromStr for RowCol {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Expected format: "(0, 1)"
        let s = s.trim();
        if !s.starts_with('(') || !s.ends_with(')') {
            anyhow::bail!("RowCol must be in format '(row, col)', got: {s}");
        }

        let inner = &s[1..s.len() - 1]; // Remove parentheses
        let parts: Vec<&str> = inner.split(',').map(|p| p.trim()).collect();

        if parts.len() != 2 {
            anyhow::bail!("RowCol must have exactly 2 components, got: {s}");
        }

        let row = parts[0]
            .parse::<usize>()
            .map_err(|e| anyhow::anyhow!("Invalid row index: {e}"))?;
        let col = parts[1]
            .parse::<usize>()
            .map_err(|e| anyhow::anyhow!("Invalid column index: {e}"))?;

        Ok(RowCol(row, col))
    }
}

impl Serialize for RowCol {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for RowCol {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        RowCol::from_str(&s).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_row_col_display() {
        let key = RowCol::new(0, 1);
        assert_eq!(key.to_string(), "(0, 1)");

        let key = RowCol::new(42, 123);
        assert_eq!(key.to_string(), "(42, 123)");
    }

    #[test]
    fn test_row_col_from_str() {
        let key: RowCol = "(0, 1)".parse().unwrap();
        assert_eq!(key, RowCol::new(0, 1));

        let key: RowCol = "(42, 123)".parse().unwrap();
        assert_eq!(key, RowCol::new(42, 123));

        // Test with extra whitespace
        let key: RowCol = "( 5 , 10 )".parse().unwrap();
        assert_eq!(key, RowCol::new(5, 10));
    }

    #[test]
    fn test_row_col_from_str_invalid() {
        assert!("0, 1".parse::<RowCol>().is_err()); // Missing parentheses
        assert!("(0, 1, 2)".parse::<RowCol>().is_err()); // Too many components
        assert!("(0)".parse::<RowCol>().is_err()); // Too few components
        assert!("(a, b)".parse::<RowCol>().is_err()); // Non-numeric
    }

    #[test]
    fn test_row_col_serialize() {
        let key = RowCol::new(0, 1);
        let serialized = serde_json::to_string(&key).unwrap();
        assert_eq!(serialized, r#""(0, 1)""#);
    }

    #[test]
    fn test_row_col_deserialize() {
        let key: RowCol = serde_json::from_str(r#""(0, 1)""#).unwrap();
        assert_eq!(key, RowCol::new(0, 1));
    }

    #[test]
    fn test_row_col_roundtrip() {
        let original = RowCol::new(42, 123);
        let serialized = serde_json::to_string(&original).unwrap();
        let deserialized: RowCol = serde_json::from_str(&serialized).unwrap();
        assert_eq!(original, deserialized);
    }
}
