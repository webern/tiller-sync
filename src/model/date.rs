//! This is our `Date` object that serializes as `YYYY-MM-DD` for best results in SQLite. When we
//! upload to the Tiller Google sheet, we need it formatted as `M/D/YYYY` per Tiller's
//! specifications.

use crate::error::{ErrorType, IntoResult, Res};
use crate::TillerError;
use anyhow::{bail, ensure, Context};
use chrono::{DateTime, FixedOffset, NaiveDateTime};
use schemars::{json_schema, JsonSchema, Schema, SchemaGenerator};
use std::borrow::Cow;
use std::fmt::{Debug, Display, Formatter};
use std::str::FromStr;
use tracing::debug;

/// A date value that serializes in YYYY-MM-DD format.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, sqlx::Type)]
#[sqlx(transparent)]
pub struct Date(String);

impl JsonSchema for Date {
    fn schema_name() -> Cow<'static, str> {
        "Date".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "format": "date",
            "pattern": "^\\d{4}-\\d{2}-\\d{2}$",
            "description": "A date in YYYY-MM-DD format (e.g., 2025-01-23)"
        })
    }
}

impl Default for Date {
    fn default() -> Self {
        Date("1999-12-31".to_string())
    }
}

impl Date {
    pub fn parse(s: impl AsRef<str>) -> Res<Self> {
        let s = s.as_ref();
        if s.contains(':') {
            Self::parse_with_chrono(s)
        } else if s.contains('/') {
            Self::parse_m_d_yyyy(s)
        } else if s.contains('-') {
            Self::parse_yyyy_mm_dd(s)
        } else {
            bail!("Expected a date eith in the format 9/30/2025 or 2025-09-31, but received {s}")
        }
    }

    /// Convenience for deserializing from the database.
    fn from_opt(o: Option<String>) -> Res<Option<Self>> {
        match o {
            None => Ok(None),
            Some(s) => Self::from_opt_s(s),
        }
    }

    /// For deserializing strings that may be empty.
    fn from_opt_s(s: impl AsRef<str>) -> Res<Option<Self>> {
        let s = s.as_ref();
        if s.is_empty() {
            Ok(None)
        } else {
            Ok(Some(Self::parse(s)?))
        }
    }

    fn parse_m_d_yyyy(s: &str) -> Res<Self> {
        let mut parts = s.split('/');
        let m = parts.next().context(format!("No month found for {s}"))?;
        let d = parts.next().context(format!("No day found for {s}"))?;
        let y = parts.next().context(format!("No year found for {s}"))?;
        ensure!(parts.next().is_none(), "Too many parts found for {s}");
        Self::from_y_m_d(y, m, d, s)
    }

    fn parse_yyyy_mm_dd(s: &str) -> Res<Self> {
        let mut parts = s.split('-');
        let y = parts.next().context(format!("No year found for {s}"))?;
        let m = parts.next().context(format!("No month found for {s}"))?;
        let d = parts.next().context(format!("No day found for {s}"))?;
        ensure!(parts.next().is_none(), "Too many parts found for {s}");
        Self::from_y_m_d(y, m, d, s)
    }

    /// This is the format that Tiller uses for categorized date.
    /// Handles multiple formats:
    /// - `M/D/YYYY H:MM:SS AM/PM` (US format) -> outputs without timezone
    /// - `YYYY-MM-DDTHH:MM:SS` (ISO without timezone) -> outputs without timezone
    /// - `YYYY-MM-DDTHH:MM:SS-08:00` (ISO with timezone offset) -> preserves timezone
    /// - `YYYY-MM-DDTHH:MM:SS.ffffffZ` (ISO with fractional seconds and Z) -> preserves Z
    fn parse_with_chrono(s: &str) -> Res<Self> {
        if s.contains('/') {
            // US format: M/D/YYYY H:MM:SS AM/PM -> no timezone
            let d = NaiveDateTime::parse_from_str(s, "%m/%d/%Y %I:%M:%S %p")
                .context(format!("Unable to parse {s} as a date"))?;
            Ok(Self(d.format("%Y-%m-%dT%H:%M:%S").to_string()))
        } else if s.ends_with('Z') || s.contains('+') || s.rfind('-').is_some_and(|i| i > 10) {
            // ISO format with timezone: preserve the offset
            // The rfind('-') > 10 check distinguishes timezone offset from date separator
            let dt = DateTime::<FixedOffset>::parse_from_rfc3339(s)
                .or_else(|_| DateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%z"))
                .or_else(|_| DateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f%z"))
                .context(format!("Unable to parse {s} as a date with timezone"))?;
            // Format with timezone offset preserved (RFC3339 style: -08:00)
            Ok(Self(dt.format("%Y-%m-%dT%H:%M:%S%:z").to_string()))
        } else {
            // ISO format without timezone
            let d = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S")
                .or_else(|_| NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f"))
                .context(format!("Unable to parse {s} as a date"))?;
            Ok(Self(d.format("%Y-%m-%dT%H:%M:%S").to_string()))
        }
    }

    fn from_y_m_d(y: &str, m: &str, d: &str, original: &str) -> Res<Self> {
        let m = m
            .parse::<i32>()
            .context(format!("Month is a bad number for {original}, m={m}"))?;
        let d = d
            .parse::<i32>()
            .context(format!("Day is a bad number for {original}, d={d}"))?;
        let mut y = y
            .parse::<i32>()
            .context(format!("Year is a bad number for {original}, y={y}"))?;
        // Yuck... but, ok, let's support two-digit dates
        if y < 100 {
            debug!("A two-digit year was interpreted to be in the 21st century: {original}");
            y += 2000;
        }
        ensure!(
            (1..=12).contains(&m),
            "Bad month value of {m} in {original}"
        );
        ensure!((1..=31).contains(&d), "Bad day value of {d} in {original}");
        ensure!(
            (1000..=9999).contains(&y),
            "Bad year value of {y} in {original}"
        );
        Ok(Self(format!("{y:04}-{m:02}-{d:02}")))
    }
}

impl TryFrom<String> for Date {
    type Error = TillerError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value).pub_result(ErrorType::Internal)
    }
}

impl TryFrom<&str> for Date {
    type Error = TillerError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value).pub_result(ErrorType::Internal)
    }
}

impl Display for Date {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl Debug for Date {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl FromStr for Date {
    type Err = TillerError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).pub_result(ErrorType::Internal)
    }
}

pub(crate) trait DateFromOpt: Sized {
    fn date_from_opt(self) -> Res<Option<Date>>;
}

impl<S> DateFromOpt for Option<S>
where
    S: AsRef<str> + Sized,
{
    fn date_from_opt(self) -> Res<Option<Date>> {
        let o = self.map(|s| s.as_ref().to_string());
        Date::from_opt(o)
    }
}

pub(crate) trait DateFromOptStr: Sized {
    fn date_from_opt_s(self) -> Res<Option<Date>>;
}

impl<S> DateFromOptStr for S
where
    S: AsRef<str> + Sized,
{
    fn date_from_opt_s(self) -> Res<Option<Date>> {
        Date::from_opt_s(self)
    }
}

pub(crate) trait DateCanBeEmptyStr {
    fn date_to_s(&self) -> String;
}

impl DateCanBeEmptyStr for Option<Date> {
    fn date_to_s(&self) -> String {
        self.as_ref().map(|d| d.to_string()).unwrap_or_default()
    }
}

impl DateCanBeEmptyStr for Option<&Date> {
    fn date_to_s(&self) -> String {
        self.map(|d| d.to_string()).unwrap_or_default()
    }
}

impl DateCanBeEmptyStr for &Option<Date> {
    fn date_to_s(&self) -> String {
        self.as_ref().map(|d| d.to_string()).unwrap_or_default()
    }
}

serde_plain::derive_deserialize_from_fromstr!(Date, "Valid date in M/D/YYYY or YYYY-MM-DD");
serde_plain::derive_serialize_from_display!(Date);

#[cfg(test)]
mod test {
    use super::*;

    fn success_case(input: &str, expected_s: &str) {
        let text = format!("Test failure parsing {input} and expecting {expected_s}");
        let expected = Date(String::from(expected_s.to_string()));
        let actual = Date::parse(&input).expect(&text);
        assert_eq!(expected, actual);

        let json_str = format!("[\"{input}\"]");
        let arr: Vec<Date> = serde_json::from_str(&json_str).expect(&format!(
            "{text}: the json '{json_str}' could not be deserialized"
        ));
        let serialized =
            serde_json::to_string(&arr).expect(&format!("{text}, unable to serialize"));
        let json_expected = format!("[\"{expected_s}\"]");
        assert_eq!(
            json_expected, serialized,
            "{text}, did not get the expected serialization"
        )
    }

    fn failure_case(input: &str) {
        let res = Date::parse(&input);
        assert!(
            res.is_err(),
            "Expected an error when parsing {input} but received Ok"
        );
        let msg = res.err().unwrap().to_string();
        let contains_input = msg.contains(input);
        assert!(
            contains_input,
            "Expected the error message when parsing {input} to contain the \
             input string, but it did not"
        );
    }

    #[test]
    fn test_parse_good_1() {
        success_case("9/30/2025", "2025-09-30");
    }

    #[test]
    fn test_parse_good_2() {
        success_case("2025-09-30", "2025-09-30");
    }

    #[test]
    fn test_parse_good_3() {
        success_case("1999-6-2", "1999-06-02");
    }

    #[test]
    fn test_parse_good_4() {
        success_case("12/000001/1932", "1932-12-01");
    }

    #[test]
    fn test_parse_good_5() {
        success_case("10/31/5", "2005-10-31");
    }

    #[test]
    fn test_parse_bad_1() {
        failure_case("99/30/2025");
    }

    #[test]
    fn test_parse_bad_2() {
        failure_case("9/32/2025")
    }

    #[test]
    fn test_parse_bad_3() {
        failure_case("foo")
    }

    // Tests for parse_with_chrono (datetime formats with colons)

    #[test]
    fn test_parse_chrono_iso_format() {
        success_case("2025-01-23T10:30:45", "2025-01-23T10:30:45");
    }

    #[test]
    fn test_parse_chrono_iso_midnight() {
        success_case("2025-12-31T00:00:00", "2025-12-31T00:00:00");
    }

    #[test]
    fn test_parse_chrono_iso_end_of_day() {
        success_case("2025-06-15T23:59:59", "2025-06-15T23:59:59");
    }

    #[test]
    fn test_parse_chrono_us_format_am() {
        success_case("01/23/2025 10:30:45 AM", "2025-01-23T10:30:45");
    }

    #[test]
    fn test_parse_chrono_us_format_pm() {
        success_case("01/23/2025 02:30:45 PM", "2025-01-23T14:30:45");
    }

    #[test]
    fn test_parse_chrono_us_format_noon() {
        success_case("07/04/2025 12:00:00 PM", "2025-07-04T12:00:00");
    }

    #[test]
    fn test_parse_chrono_us_format_midnight() {
        success_case("12/25/2025 12:00:00 AM", "2025-12-25T00:00:00");
    }

    #[test]
    fn test_parse_chrono_bad_iso() {
        failure_case("2025-13-01T10:30:45");
    }

    #[test]
    fn test_parse_chrono_bad_us_format() {
        failure_case("13/01/2025 10:30:45 AM");
    }

    #[test]
    fn test_parse_chrono_bad_time() {
        failure_case("2025-01-23T25:00:00");
    }

    // Tests for timezone preservation

    #[test]
    fn test_parse_chrono_with_negative_offset() {
        // Input has -0800 offset, output should have -08:00 (RFC3339 style)
        success_case("2024-12-31T06:17:17-0800", "2024-12-31T06:17:17-08:00");
    }

    #[test]
    fn test_parse_chrono_with_positive_offset() {
        success_case("2025-01-23T15:30:00+0530", "2025-01-23T15:30:00+05:30");
    }

    #[test]
    fn test_parse_chrono_with_rfc3339_offset() {
        // Already in RFC3339 format with colon
        success_case("2025-01-23T10:00:00-05:00", "2025-01-23T10:00:00-05:00");
    }

    #[test]
    fn test_parse_chrono_with_z_suffix() {
        success_case("2025-01-23T10:00:00Z", "2025-01-23T10:00:00+00:00");
    }

    #[test]
    fn test_parse_chrono_with_fractional_seconds_and_z() {
        // Fractional seconds should be dropped, Z converted to +00:00
        success_case("2025-01-23T10:00:00.123456Z", "2025-01-23T10:00:00+00:00");
    }

    #[test]
    fn test_parse_chrono_with_fractional_seconds_and_offset() {
        success_case(
            "2024-12-31T06:17:17.465339-08:00",
            "2024-12-31T06:17:17-08:00",
        );
    }
}
