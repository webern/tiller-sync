use serde::de::Error as SerdeError;
use serde::ser::SerializeSeq;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::convert::Infallible;
use std::error::Error as StdError;
use std::fmt::{Display, Formatter};
use std::str::FromStr;

#[derive(Debug, Default, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct MappingError(String);

impl Display for MappingError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl StdError for MappingError {}

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct Mapping {
    headers: Vec<Header>,
    columns: Vec<Column>,
    header_map: HashMap<Header, usize>,
    column_map: HashMap<Column, usize>,
}

impl Mapping {
    /// Create a new `ColumnMap` from a list of header string. The column names will be created by
    /// converting header strings to camel_case.
    pub fn new<S, I>(headers: I) -> Result<Self, MappingError>
    where
        S: Into<String>,
        I: IntoIterator<Item = S>,
    {
        let headers: Vec<Header> = headers.into_iter().map(|s| s.into().into()).collect();
        let columns = headers
            .iter()
            .map(|h| Column::new(to_camel_case(h)))
            .collect::<Result<Vec<Column>, MappingError>>()?;

        let header_map: HashMap<Header, usize> = headers
            .iter()
            .enumerate()
            .map(|(idx, key)| (key.to_owned(), idx))
            .collect();

        let column_map: HashMap<Column, usize> = columns
            .iter()
            .enumerate()
            .map(|(idx, key)| (key.to_owned(), idx))
            .collect();

        let expected_length = headers.len();

        if header_map.len() != expected_length {
            return Err(MappingError(String::from("Encountered a duplicate header")));
        }

        if column_map.len() != expected_length {
            return Err(MappingError(String::from(
                "Encountered a duplicate column name \
                (two or more headers resulted in the same snake_case conversion)",
            )));
        }

        Ok(Self {
            headers,
            columns,
            header_map,
            column_map,
        })
    }

    pub fn len(&self) -> usize {
        self.headers.len()
    }

    pub fn _is_empty(&self) -> bool {
        self.headers.is_empty()
    }

    pub fn headers(&self) -> &[Header] {
        &self.headers
    }

    pub fn columns(&self) -> &[Column] {
        &self.columns
    }

    pub fn _header_index(&self, header: impl Into<Header>) -> Option<usize> {
        let h = header.into();
        self.header_map.get(&h).cloned()
    }
}

impl Serialize for Mapping {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(self.headers.len()))?;
        for header in &self.headers {
            seq.serialize_element(header.as_ref())?;
        }
        seq.end()
    }
}

impl<'de> Deserialize<'de> for Mapping {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let items: Vec<String> = Vec::deserialize(deserializer)?;
        let mapping = Mapping::new(items).map_err(D::Error::custom)?;
        Ok(mapping)
    }
}

/// Represents a header in the Google sheet, for example, `Account #`
#[derive(Default, Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Header(String);

impl AsRef<str> for Header {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

impl<S: Into<String>> From<S> for Header {
    fn from(value: S) -> Self {
        Self(value.into())
    }
}

impl FromStr for Header {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(s.into())
    }
}

/// Represents a column name in the SQLite database, for example, `account_number`
#[derive(Default, Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Column(String);

impl AsRef<str> for Column {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

impl FromStr for Column {
    type Err = MappingError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        validate_column_name(s)?;
        Ok(Self(s.into()))
    }
}

impl Column {
    pub fn new(s: impl Into<String>) -> Result<Self, MappingError> {
        let s = s.into();
        validate_column_name(&s)?;
        Ok(Self(s))
    }
}

fn to_camel_case(s: impl AsRef<str>) -> String {
    if s.as_ref().is_empty() {
        return "no_name".to_string();
    }
    let lower = s
        .as_ref()
        .to_lowercase()
        .replace(' ', "_")
        .replace('#', "number");
    let alphanumeric: String = lower
        .chars()
        .filter(|&c| c.is_ascii_alphanumeric() || c == '_')
        .collect();
    let first_char = lower.chars().next().unwrap_or('a');
    if !first_char.is_ascii_alphabetic() {
        format!("x_{alphanumeric}")
    } else {
        alphanumeric
    }
}

fn validate_column_name(s: impl AsRef<str>) -> std::result::Result<(), MappingError> {
    let s = s.as_ref();
    let mut chars = s.chars();
    match chars.next() {
        None => {
            return Err(MappingError(String::from(
                "A column name must not be zero length",
            )))
        }
        Some(c) => {
            if !c.is_ascii_alphabetic() || !c.is_ascii_lowercase() {
                return Err(MappingError(format!(
                    "A column name must start with an ascii lowercase letter, \
                    but '{s}' starts with '{c}'"
                )));
            }
        }
    }

    if let Some(bad) = chars.find(|&c| !is_valid_column_name_char(c)) {
        return Err(MappingError(format!(
            "A column name must be lowercase ascii alphanumeric with underscores. \
            '{s}' has illegal char '{bad}'"
        )));
    }

    Ok(())
}

fn is_valid_column_name_char(c: char) -> bool {
    if c == '_' {
        return true;
    }
    if c.is_ascii_digit() {
        return true;
    }
    if c.is_ascii_lowercase() && c.is_ascii_alphabetic() {
        return true;
    }
    false
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::model::transaction::ACCOUNT_NUMBER_STR;

    #[test]
    fn test_legal_column_name_chars() {
        assert!(is_valid_column_name_char('a'));
        assert!(is_valid_column_name_char('1'));
        assert!(is_valid_column_name_char('_'));
    }

    #[test]
    fn test_illegal_column_name_chars() {
        assert!(!is_valid_column_name_char('#'));
        assert!(!is_valid_column_name_char(' '));
        assert!(!is_valid_column_name_char('X'));
        assert!(!is_valid_column_name_char('\0'));
    }

    #[test]
    fn test_valid_column_name_1() {
        validate_column_name("header_1").unwrap();
    }

    #[test]
    fn test_valid_column_name_2() {
        validate_column_name("snake_case_is_fine_123_").unwrap();
    }

    #[test]
    fn test_invalid_column_name_1() {
        assert!(validate_column_name("1leading_numeral").is_err());
    }

    #[test]
    fn test_invalid_column_name_2() {
        assert!(validate_column_name("_leading_underscore").is_err());
    }

    #[test]
    fn test_invalid_column_name_3() {
        assert!(validate_column_name("has space").is_err());
    }

    #[test]
    fn test_invalid_column_name_4() {
        assert!(validate_column_name("upper_Case").is_err());
    }

    #[test]
    fn test_to_camel_case_01() {
        let original = ACCOUNT_NUMBER_STR;
        let expected = "account_number";
        let actual = to_camel_case(original);
        assert_eq!(expected, actual);
        validate_column_name(actual).unwrap();
    }

    #[test]
    fn test_to_camel_case_02() {
        let original = "123";
        let expected = "x_123";
        let actual = to_camel_case(original);
        assert_eq!(expected, actual);
        validate_column_name(actual).unwrap();
    }

    #[test]
    fn test_mapping_serde() {
        let original_json = r##"["Header 1","Category","Default Something"]"##;
        let mapping: Mapping = serde_json::from_str(&original_json).unwrap();
        let serialized = serde_json::to_string(&mapping).unwrap();
        assert_eq!(original_json, serialized);

        assert_eq!(
            mapping.columns,
            vec!["header_1", "category", "default_something"]
                .into_iter()
                .map(|s| Column::new(s).unwrap())
                .collect::<Vec<Column>>()
        );
    }
}
