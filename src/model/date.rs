//! This is our `Date` object that serializes as `YYYY-MM-DD` for best results in SQLite. When we
//! upload to the Tiller Google sheet, we need it formatted as `M/D/YYYY` per Tiller's
//! specifications.

use crate::error::{ErrorType, IntoResult, Res};
use crate::TillerError;
use anyhow::bail;
use chrono::{DateTime, FixedOffset, NaiveDate, NaiveDateTime};
use schemars::{json_schema, JsonSchema, Schema, SchemaGenerator};
use sqlx::encode::IsNull;
use sqlx::error::BoxDynError;
use sqlx::sqlite::{SqliteArgumentsBuffer, SqliteTypeInfo, SqliteValueRef};
use sqlx::{Decode, Encode, Sqlite, Type};
use std::borrow::Cow;
use std::fmt::{Debug, Display, Formatter};
use std::str::FromStr;

/// A date value that serializes in YYYY-MM-DD format.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum Date {
    Naive(NaiveDate),
    NaiveTime(NaiveDateTime),
    Timestamp(DateTime<FixedOffset>),
}

impl Type<Sqlite> for Date {
    fn type_info() -> SqliteTypeInfo {
        <String as Type<Sqlite>>::type_info()
    }

    fn compatible(ty: &SqliteTypeInfo) -> bool {
        <String as Type<Sqlite>>::compatible(ty)
    }
}

impl Encode<'_, Sqlite> for Date {
    fn encode_by_ref(&self, buf: &mut SqliteArgumentsBuffer) -> Result<IsNull, BoxDynError> {
        Encode::<Sqlite>::encode(self.to_string(), buf)
    }
}

impl Decode<'_, Sqlite> for Date {
    fn decode(value: SqliteValueRef<'_>) -> Result<Self, BoxDynError> {
        let s = <String as Decode<Sqlite>>::decode(value)?;
        Date::parse(&s).map_err(|e| e.into())
    }
}

impl JsonSchema for Date {
    fn schema_name() -> Cow<'static, str> {
        "Date".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "format": "date",
            "description": "A date in YYYY-MM-DD format (e.g., 2025-01-23), or ISO 8601 RFC 3339"
        })
    }
}

impl Default for Date {
    fn default() -> Self {
        Date::Naive(NaiveDate::from_ymd_opt(1999, 12, 31).unwrap_or_default())
    }
}

impl Date {
    pub fn parse(s: impl AsRef<str>) -> Res<Self> {
        let s = s.as_ref();

        if let Some(d) = NAIVE_DATE_FORMATS
            .iter()
            .find_map(|&fmt| NaiveDate::parse_from_str(s, fmt).ok())
        {
            return Ok(Date::Naive(d));
        }

        if let Some(d) = NAIVE_DATE_TIME_FORMATS
            .iter()
            .find_map(|&fmt| NaiveDateTime::parse_from_str(s, fmt).ok())
        {
            return Ok(Date::NaiveTime(d));
        }

        if let Some(d) = DATE_TIME_FORMATS
            .iter()
            .find_map(|&fmt| DateTime::parse_from_str(s, fmt).ok())
        {
            return Ok(Date::Timestamp(d));
        }

        // Try RFC 3339 (handles Z suffix and standard timezone offsets)
        if let Ok(d) = DateTime::parse_from_rfc3339(s) {
            return Ok(Date::Timestamp(d));
        }

        bail!("Unable to parse {s} as a date")
    }

    /// Print the date in the format that Tiller uses in the spreadsheet.
    /// - `6/30/2025` (i.e. US Date)
    /// - `01/21/2026 4:37:48 AM` (i.e. US Date plush AM/PM time without timezone)
    ///
    /// Note: we will lose time zone information in the Google sheet so please don't expect it or
    /// use it for anything important!
    pub(crate) fn to_sheet_string(&self, y: Y) -> String {
        match y {
            Y::Y2 => match self {
                Date::Naive(d) => d.format("%-m/%-d/%y").to_string(),
                Date::NaiveTime(d) => d.format("%-m/%-d/%y %-I:%M:%S %p").to_string(),
                Date::Timestamp(d) => d.format("%-m/%-d/%y %-I:%M:%S %p").to_string(),
            },
            Y::Y4 => match self {
                Date::Naive(d) => d.format("%m/%d/%Y").to_string(),
                Date::NaiveTime(d) => d.format("%m/%d/%Y %I:%M:%S %p").to_string(),
                Date::Timestamp(d) => d.format("%m/%d/%Y %I:%M:%S %p").to_string(),
            },
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
}

/// Whether the date should be printed with a 2-digit or 4-digit year.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(crate) enum Y {
    /// Two digit year
    Y2,
    /// Four digit year
    Y4,
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
        let s = match self {
            Date::Naive(d) => d.format("%Y-%m-%d").to_string(),
            Date::NaiveTime(d) => d.format("%Y-%m-%dT%H:%M:%S").to_string(),
            Date::Timestamp(d) => d.to_rfc3339(),
        };
        Display::fmt(&s, f)
    }
}

impl FromStr for Date {
    type Err = TillerError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).pub_result(ErrorType::Internal)
    }
}

/// This trait allows us to parse a date from an option conveniently.
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

/// This allows us to conveniently parse a Date from a string that might be empty, where we want to
/// treat the empty string as `None`.
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

/// This is a convenient serialization function to print out the date for Google sheet upload where
/// we want it in the US Date format (`5/30/2025`)) to match Tiller, and if the Option is `None`
/// then we want an empty string.
pub(crate) trait DateToSheetStr {
    fn d_to_s(&self, y: Y) -> String;
}

impl DateToSheetStr for Date {
    fn d_to_s(&self, y: Y) -> String {
        self.to_sheet_string(y)
    }
}

impl DateToSheetStr for &Date {
    fn d_to_s(&self, y: Y) -> String {
        self.to_sheet_string(y)
    }
}

impl DateToSheetStr for Option<Date> {
    fn d_to_s(&self, y: Y) -> String {
        self.as_ref()
            .map(|d| d.to_sheet_string(y))
            .unwrap_or_default()
    }
}

impl DateToSheetStr for Option<&Date> {
    fn d_to_s(&self, y: Y) -> String {
        self.map(|d| d.to_sheet_string(y)).unwrap_or_default()
    }
}

impl DateToSheetStr for &Option<Date> {
    fn d_to_s(&self, y: Y) -> String {
        self.as_ref()
            .map(|d| d.to_sheet_string(y))
            .unwrap_or_default()
    }
}

serde_plain::derive_deserialize_from_fromstr!(Date, "Valid date in M/D/YYYY or YYYY-MM-DD");
serde_plain::derive_serialize_from_display!(Date);

const NAIVE_DATE_FORMATS: [&str; 23] = [
    // ISO 8601 / RFC 3339 variants
    "%Y-%m-%d", // 2025-01-24
    "%Y%m%d",   // 20250124
    // US formats
    "%m/%d/%y", // 01/24/25
    "%m-%d-%y", // 01-24-25
    "%m/%d/%Y", // 01/24/2025
    "%m-%d-%Y", // 01-24-2025
    "%m.%d.%Y", // 01.24.2025
    // European formats
    "%d/%m/%y", // 24/01/25
    "%d-%m-%y", // 24-01-25
    "%d.%m.%y", // 24.01.25
    "%d/%m/%Y", // 24/01/2025
    "%d-%m-%Y", // 24-01-2025
    "%d.%m.%Y", // 24.01.2025
    // Written months (English)
    "%d-%b-%y",  // 24-Jan-25
    "%B %d, %Y", // January 24, 2025
    "%b %d, %Y", // Jan 24, 2025
    "%d %B %Y",  // 24 January 2025
    "%d %b %Y",  // 24 Jan 2025
    "%B %d %Y",  // January 24 2025
    "%b %d %Y",  // Jan 24 2025
    "%d-%b-%Y",  // 24-Jan-2025
    // Misc
    "%Y/%m/%d", // 2025/01/24
    "%Y.%m.%d", // 2025.01.24
];

const NAIVE_DATE_TIME_FORMATS: [&str; 12] = [
    // ISO 8601 / RFC 3339 variants
    "%Y-%m-%dT%H:%M:%S",    // 2025-01-24T14:30:00
    "%Y-%m-%dT%H:%M:%S%.f", // 2025-01-24T14:30:00.123456
    "%Y%m%dT%H%M%S",        // 20250124T143000
    // With time (common)
    "%Y-%m-%d %H:%M:%S",    // 2025-01-24 14:30:00
    "%Y-%m-%d %H:%M",       // 2025-01-24 14:30
    "%m/%d/%y %H:%M:%S",    // 01/24/25 14:30:00
    "%m/%d/%y %I:%M:%S %p", // 01/24/25 02:30:00 PM
    "%d/%m/%y %H:%M:%S",    // 24/01/25 14:30:00
    "%m/%d/%Y %H:%M:%S",    // 01/24/2025 14:30:00
    "%m/%d/%Y %I:%M:%S %p", // 01/24/2025 02:30:00 PM
    "%d/%m/%Y %H:%M:%S",    // 24/01/2025 14:30:00
    // Unix/log style
    "%b %d %H:%M:%S %Y", // Jan 24 14:30:00 2025
];

const DATE_TIME_FORMATS: [&str; 12] = [
    // ISO 8601 / RFC 3339 variants
    "%Y-%m-%dT%H:%M:%S%Z",    // 2025-01-24T14:30:00Z
    "%Y-%m-%dT%H:%M:%S%z",    // 2025-01-24T14:30:00+0000
    "%Y-%m-%dT%H:%M:%S%.f%Z", // 2025-01-24T14:30:00.123456Z
    "%Y-%m-%dT%H:%M:%S%.f%z", // 2025-01-24T14:30:00.123456+0000
    "%Y%m%dT%H%M%S%Z",        // 20250124T143000Z
    "%Y%m%dT%H%M%S%z",        // 20250124T143000+0000
    "%Y-%m-%d %H:%M:%S%Z",    // 2025-01-24 14:30:00Z
    "%Y-%m-%d %H:%M:%S%z",    // 2025-01-24 14:30:00+0000
    "%Y-%m-%d %H:%M:%S%.f%Z", // 2025-01-24 14:30:00.123456Z
    "%Y-%m-%d %H:%M:%S%.f%z", // 2025-01-24 14:30:00.123456+0000
    "%Y%m%d %H%M%S%Z",        // 20250124 143000Z
    "%Y%m%d %H%M%S%z",        // 20250124 143000+0000
];

#[cfg(test)]
mod test {
    use super::*;

    fn success_case(input: &str, expected: &str, sheet_y2: &str, sheet_y4: &str) {
        let text = format!("Test failure parsing {input} and expecting {expected}");
        let actual = Date::parse(&input).expect(&text);
        assert_eq!(expected, actual.to_string());

        let json_str = format!("[\"{input}\"]");
        let arr: Vec<Date> = serde_json::from_str(&json_str).expect(&format!(
            "{text}: the json '{json_str}' could not be deserialized"
        ));
        let serialized =
            serde_json::to_string(&arr).expect(&format!("{text}, unable to serialize"));
        let json_expected = format!("[\"{expected}\"]");
        assert_eq!(
            json_expected, serialized,
            "{text}, did not get the expected serialization"
        );

        let actual_sheet_y2 = actual.d_to_s(Y::Y2);
        assert_eq!(
            sheet_y2, actual_sheet_y2,
            "{text} Sheet Y2 formatting is incorrect"
        );
        let actual_sheet_y4 = actual.d_to_s(Y::Y4);
        assert_eq!(
            sheet_y4, actual_sheet_y4,
            "{text} Sheet Y4 formatting is incorrect"
        );
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
        success_case("9/30/2025", "2025-09-30", "9/30/25", "09/30/2025");
    }

    #[test]
    fn test_parse_good_2() {
        success_case("2025-09-30", "2025-09-30", "9/30/25", "09/30/2025");
    }

    #[test]
    fn test_parse_good_3() {
        success_case("1999-6-2", "1999-06-02", "6/2/99", "06/02/1999");
    }

    #[test]
    fn test_parse_bad_leading_zeros() {
        failure_case("12/000001/1932");
    }

    #[test]
    fn test_parse_good_5() {
        // 2-digit years are interpreted by chrono (5 -> 2005)
        success_case("10/31/05", "2005-10-31", "10/31/05", "10/31/2005");
    }

    #[test]
    fn test_parse_good_6() {
        // 2-digit years are interpreted by chrono (5 -> 2005)
        success_case("10/1/25", "2025-10-01", "10/1/25", "10/01/2025");
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
        success_case(
            "2025-01-23T10:30:45",
            "2025-01-23T10:30:45",
            "1/23/25 10:30:45 AM",
            "01/23/2025 10:30:45 AM",
        );
    }

    #[test]
    fn test_parse_chrono_iso_midnight() {
        success_case(
            "2025-12-31T00:00:00",
            "2025-12-31T00:00:00",
            "12/31/25 12:00:00 AM",
            "12/31/2025 12:00:00 AM",
        );
    }

    #[test]
    fn test_parse_chrono_iso_end_of_day() {
        success_case(
            "2025-06-15T23:59:59",
            "2025-06-15T23:59:59",
            "6/15/25 11:59:59 PM",
            "06/15/2025 11:59:59 PM",
        );
    }

    #[test]
    fn test_parse_chrono_us_format_am() {
        success_case(
            "01/23/2025 10:30:45 AM",
            "2025-01-23T10:30:45",
            "1/23/25 10:30:45 AM",
            "01/23/2025 10:30:45 AM",
        );
    }

    #[test]
    fn test_parse_chrono_us_format_pm() {
        success_case(
            "01/23/2025 02:30:45 PM",
            "2025-01-23T14:30:45",
            "1/23/25 2:30:45 PM",
            "01/23/2025 02:30:45 PM",
        );
    }

    #[test]
    fn test_parse_chrono_us_format_noon() {
        success_case(
            "07/04/2025 12:00:00 PM",
            "2025-07-04T12:00:00",
            "7/4/25 12:00:00 PM",
            "07/04/2025 12:00:00 PM",
        );
    }

    #[test]
    fn test_parse_chrono_us_format_midnight() {
        success_case(
            "12/25/2025 12:00:00 AM",
            "2025-12-25T00:00:00",
            "12/25/25 12:00:00 AM",
            "12/25/2025 12:00:00 AM",
        );
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
        success_case(
            "2024-12-31T06:17:17-0800",
            "2024-12-31T06:17:17-08:00",
            "12/31/24 6:17:17 AM",
            "12/31/2024 06:17:17 AM",
        );
    }

    #[test]
    fn test_parse_chrono_with_positive_offset() {
        success_case(
            "2025-01-23T15:30:00+0530",
            "2025-01-23T15:30:00+05:30",
            "1/23/25 3:30:00 PM",
            "01/23/2025 03:30:00 PM",
        );
    }

    #[test]
    fn test_parse_chrono_with_rfc3339_offset() {
        // Already in RFC3339 format with colon
        success_case(
            "2025-01-23T10:00:00-05:00",
            "2025-01-23T10:00:00-05:00",
            "1/23/25 10:00:00 AM",
            "01/23/2025 10:00:00 AM",
        );
    }

    #[test]
    fn test_parse_chrono_with_z_suffix() {
        success_case(
            "2025-01-23T10:00:00Z",
            "2025-01-23T10:00:00+00:00",
            "1/23/25 10:00:00 AM",
            "01/23/2025 10:00:00 AM",
        );
    }

    #[test]
    fn test_parse_chrono_with_fractional_seconds_and_z() {
        // Fractional seconds are preserved in ISO, dropped in sheet format
        success_case(
            "2025-01-23T10:00:00.123456Z",
            "2025-01-23T10:00:00.123456+00:00",
            "1/23/25 10:00:00 AM",
            "01/23/2025 10:00:00 AM",
        );
    }

    #[test]
    fn test_parse_chrono_with_fractional_seconds_and_offset() {
        // Fractional seconds are preserved in ISO, dropped in sheet format
        success_case(
            "2024-12-31T06:17:17.465339-08:00",
            "2024-12-31T06:17:17.465339-08:00",
            "12/31/24 6:17:17 AM",
            "12/31/2024 06:17:17 AM",
        );
    }
}
