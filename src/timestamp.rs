//! Timestamp and timestamp delta types.
//!
//! Useful for internal representation of time, and exposes methods for
//! conversion to and from `DateTime`.
//!
//! # Motivation
//!
//! Using `NanoTimestamp` instead of formatted timestamps (like strings or
//! DateTime) provides several benefits:
//!
//! - **Space efficiency**: Store timestamps as a single i64 nanosecond value.
//! - **Type safety**: Make illegal states unrepresentable by enforcing valid
//!   timestamps.
//! - **Performance**: Enable fast timestamp comparisons and arithmetic
//!   operations.
//! - **Timezone safety**: Internal representation is always UTC, preventing
//!   timezone confusion.
//! - **Precision**: Nanosecond precision for high-accuracy time measurements.
//! - **Conversion**: Easy conversion to/from DateTime when needed for display
//!   or external APIs.
//!
//! It is recommended to work with `NanoTimestamp` and `NanoDelta` in your
//! application logic, and only convert to `DateTime` when displaying timestamps
//! to the user or sending them to an external API.
//!
//! # Technical considerations
//!
//! The type `i64` was chosen over `u64` to allow for negative timestamps, which
//! are useful for representing time deltas.

use core::fmt;
use std::{
    convert::TryFrom,
    fmt::{Debug, Display},
    num::ParseIntError,
    ops::{Add, Sub},
    str::FromStr,
};

use chrono::{DateTime, FixedOffset, Local, TimeDelta, TimeZone, Utc};
use thiserror::Error;
use zeroize::Zeroize;
/// A timestamp in nanoseconds in the UTC timezone.
///
/// The dates that can be represented as nanoseconds are between
/// 1677-09-21T00:12:43.145224192 and 2262-04-11T23:47:16.854775807.
#[derive(
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Default,
    serde::Serialize,
    serde::Deserialize,
    Hash,
    Zeroize,
)]
pub struct NanoTimestamp(i64);

/// A timestamp delta (duration) in nanoseconds.
///
/// Any time you subtract two timestamps, you get a `NanoDelta`.
#[derive(
    Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, serde::Serialize, serde::Deserialize, Hash,
)]
pub struct NanoDelta(i64);

/// Error type for timestamp conversion operations
#[derive(Debug, Error)]
pub enum TimestampError {
    #[error("Timestamp overflow: {0}")]
    Overflow(String),
    #[error("Timestamp parse error: {0}")]
    Parse(#[from] chrono::ParseError),
    #[error("Bytes mismatch - expected {expected}, got {actual}")]
    ConversionError { expected: usize, actual: usize },
}

// Constants for conversion factors
pub const NANOS_PER_MICRO: i64 = 1_000;
pub const NANOS_PER_MILLI: i64 = NANOS_PER_MICRO * 1_000;
pub const NANOS_PER_SECOND: i64 = NANOS_PER_MILLI * 1_000;
pub const NANOS_PER_MINUTE: i64 = NANOS_PER_SECOND * 60;
pub const NANOS_PER_HOUR: i64 = NANOS_PER_MINUTE * 60;
pub const NANOS_PER_DAY: i64 = NANOS_PER_HOUR * 24;

impl NanoTimestamp {
    pub const fn zero() -> Self {
        Self(0)
    }
    pub const fn as_nanos(&self) -> i64 {
        self.0
    }
    pub const fn as_micros(&self) -> i64 {
        self.0 / NANOS_PER_MICRO
    }
    pub const fn as_millis(&self) -> i64 {
        self.0 / NANOS_PER_MILLI
    }
    pub const fn as_secs(&self) -> i64 {
        self.0 / NANOS_PER_SECOND
    }
    pub const fn as_minutes(&self) -> i64 {
        self.0 / NANOS_PER_MINUTE
    }
    pub const fn as_hours(&self) -> i64 {
        self.0 / NANOS_PER_HOUR
    }
    pub const fn as_days(&self) -> i64 {
        self.0 / NANOS_PER_DAY
    }
    pub fn as_rfc2822(&self) -> String {
        self.as_utc().to_rfc2822()
    }
    pub fn as_rfc3339(&self) -> String {
        self.as_utc().to_rfc3339()
    }

    pub const fn from_nanos(nanos: i64) -> Self {
        Self(nanos)
    }
    pub const fn from_micros_safe(micros: i64) -> Self {
        Self(micros * NANOS_PER_MICRO)
    }
    pub const fn from_millis_safe(millis: i64) -> Self {
        Self(millis * NANOS_PER_MILLI)
    }
    pub const fn from_secs_safe(secs: i64) -> Self {
        Self(secs * NANOS_PER_SECOND)
    }
    pub const fn from_minutes_safe(minutes: i64) -> Self {
        Self(minutes * NANOS_PER_MINUTE)
    }
    pub const fn from_hours_safe(hours: i64) -> Self {
        Self(hours * NANOS_PER_HOUR)
    }
    pub const fn from_days_safe(days: i64) -> Self {
        Self(days * NANOS_PER_DAY)
    }
    pub fn from_micros(micros: i64) -> Result<Self, TimestampError> {
        micros
            .checked_mul(NANOS_PER_MICRO)
            .map(Self)
            .ok_or_else(|| TimestampError::Overflow("microseconds conversion overflowed".into()))
    }
    pub fn from_millis(millis: i64) -> Result<Self, TimestampError> {
        millis
            .checked_mul(NANOS_PER_MILLI)
            .map(Self)
            .ok_or_else(|| TimestampError::Overflow("milliseconds conversion overflowed".into()))
    }
    pub fn from_secs(secs: i64) -> Result<Self, TimestampError> {
        secs.checked_mul(NANOS_PER_SECOND)
            .map(Self)
            .ok_or_else(|| TimestampError::Overflow("seconds conversion overflowed".into()))
    }
    pub fn from_minutes(minutes: i64) -> Result<Self, TimestampError> {
        minutes
            .checked_mul(NANOS_PER_MINUTE)
            .map(Self)
            .ok_or_else(|| TimestampError::Overflow("minutes conversion overflowed".into()))
    }
    pub fn from_hours(hours: i64) -> Result<Self, TimestampError> {
        hours
            .checked_mul(NANOS_PER_HOUR)
            .map(Self)
            .ok_or_else(|| TimestampError::Overflow("hours conversion overflowed".into()))
    }
    pub fn from_days(days: i64) -> Result<Self, TimestampError> {
        days.checked_mul(NANOS_PER_DAY)
            .map(Self)
            .ok_or_else(|| TimestampError::Overflow("days conversion overflowed".into()))
    }

    pub fn from_rfc2822(rfc2822: &str) -> Result<Self, TimestampError> {
        let dt = DateTime::<FixedOffset>::parse_from_rfc2822(rfc2822)?;
        dt.timestamp_nanos_opt().map(Self).ok_or_else(|| {
            TimestampError::Overflow("DateTime value out of i64 nanosecond range".into())
        })
    }

    pub fn from_rfc3339(rfc3339: &str) -> Result<Self, TimestampError> {
        let dt = DateTime::<FixedOffset>::parse_from_rfc3339(rfc3339)?;
        dt.timestamp_nanos_opt().map(Self).ok_or_else(|| {
            TimestampError::Overflow("DateTime value out of i64 nanosecond range".into())
        })
    }

    pub fn as_utc(&self) -> DateTime<Utc> {
        DateTime::<Utc>::from(*self)
    }

    pub fn as_le_bytes(&self) -> [u8; 8] {
        self.0.to_le_bytes()
    }

    pub fn from_le_bytes(bytes: [u8; 8]) -> Self {
        Self(i64::from_le_bytes(bytes))
    }
}

impl NanoDelta {
    pub const fn zero() -> Self {
        Self(0)
    }
    pub const fn as_days(&self) -> i64 {
        NanoTimestamp::from_nanos(self.0).as_days()
    }
    pub const fn as_hours(&self) -> i64 {
        NanoTimestamp::from_nanos(self.0).as_hours()
    }
    pub const fn as_minutes(&self) -> i64 {
        NanoTimestamp::from_nanos(self.0).as_minutes()
    }
    pub const fn as_secs(&self) -> i64 {
        NanoTimestamp::from_nanos(self.0).as_secs()
    }
    pub const fn as_millis(&self) -> i64 {
        NanoTimestamp::from_nanos(self.0).as_millis()
    }
    pub const fn as_micros(&self) -> i64 {
        NanoTimestamp::from_nanos(self.0).as_micros()
    }
    pub const fn as_nanos(&self) -> i64 {
        self.0
    }

    pub fn from_days(days: i64) -> Result<Self, TimestampError> {
        NanoTimestamp::from_days(days).map(|ts| Self(ts.0))
    }
    pub const fn from_days_safe(days: i64) -> Self {
        Self(NanoTimestamp::from_days_safe(days).0)
    }
    pub fn from_hours(hours: i64) -> Result<Self, TimestampError> {
        NanoTimestamp::from_hours(hours).map(|ts| Self(ts.0))
    }
    pub const fn from_hours_safe(hours: i64) -> Self {
        Self(NanoTimestamp::from_hours_safe(hours).0)
    }
    pub fn from_minutes(minutes: i64) -> Result<Self, TimestampError> {
        NanoTimestamp::from_minutes(minutes).map(|ts| Self(ts.0))
    }
    pub const fn from_minutes_safe(minutes: i64) -> Self {
        Self(NanoTimestamp::from_minutes_safe(minutes).0)
    }
    pub fn from_secs(secs: i64) -> Result<Self, TimestampError> {
        NanoTimestamp::from_secs(secs).map(|ts| Self(ts.0))
    }
    pub const fn from_secs_safe(secs: i64) -> Self {
        Self(NanoTimestamp::from_secs_safe(secs).0)
    }
    pub fn from_millis(millis: i64) -> Result<Self, TimestampError> {
        NanoTimestamp::from_millis(millis).map(|ts| Self(ts.0))
    }
    pub const fn from_millis_safe(millis: i64) -> Self {
        Self(NanoTimestamp::from_millis_safe(millis).0)
    }
    pub fn from_micros(micros: i64) -> Result<Self, TimestampError> {
        NanoTimestamp::from_micros(micros).map(|ts| Self(ts.0))
    }
    pub const fn from_micros_safe(micros: i64) -> Self {
        Self(NanoTimestamp::from_micros_safe(micros).0)
    }
    pub const fn from_nanos(nanos: i64) -> Self {
        Self(nanos)
    }
}

impl Display for NanoTimestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Debug for NanoTimestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ns={} rfc3339={}", self.0, self.as_rfc3339())
    }
}

impl From<i64> for NanoTimestamp {
    fn from(nanos: i64) -> Self {
        Self(nanos)
    }
}

impl Add<TimeDelta> for NanoTimestamp {
    type Output = NanoTimestamp;

    fn add(self, rhs: TimeDelta) -> Self::Output {
        NanoTimestamp::from(
            self.0
                + rhs
                    .num_nanoseconds()
                    .expect("TimeDelta duration is too large to be represented as i64 nanoseconds and will overflow"),
        )
    }
}
impl Add<NanoTimestamp> for NanoTimestamp {
    type Output = NanoTimestamp;

    fn add(self, rhs: NanoTimestamp) -> Self::Output {
        NanoTimestamp::from(self.0 + rhs.0)
    }
}
impl Add<NanoDelta> for NanoTimestamp {
    type Output = NanoTimestamp;

    fn add(self, rhs: NanoDelta) -> Self::Output {
        NanoTimestamp::from(self.0 + rhs.0)
    }
}

impl Sub<TimeDelta> for NanoTimestamp {
    type Output = NanoDelta;

    fn sub(self, rhs: TimeDelta) -> Self::Output {
        NanoDelta::from(
            self.0
                - rhs
                    .num_nanoseconds()
                    .expect("TimeDelta duration is too large to be represented as i64 nanoseconds and will overflow"),
        )
    }
}
impl Sub<NanoTimestamp> for NanoTimestamp {
    type Output = NanoDelta;

    fn sub(self, rhs: NanoTimestamp) -> Self::Output {
        NanoDelta::from(self.0 - rhs.0)
    }
}
impl Sub<NanoDelta> for NanoTimestamp {
    type Output = NanoTimestamp;

    fn sub(self, rhs: NanoDelta) -> Self::Output {
        NanoTimestamp::from(self.0 - rhs.0)
    }
}

impl FromStr for NanoTimestamp {
    type Err = ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let nanos = i64::from_str(s)?;
        Ok(NanoTimestamp::from(nanos))
    }
}

impl TryFrom<DateTime<Utc>> for NanoTimestamp {
    type Error = TimestampError;

    fn try_from(dt: DateTime<Utc>) -> Result<Self, Self::Error> {
        dt.timestamp_nanos_opt().map(Self).ok_or_else(|| {
            TimestampError::Overflow("DateTime<Utc> value out of i64 nanosecond range".into())
        })
    }
}

impl TryFrom<DateTime<Local>> for NanoTimestamp {
    type Error = TimestampError;

    fn try_from(dt: DateTime<Local>) -> Result<Self, Self::Error> {
        dt.with_timezone(&Utc)
            .timestamp_nanos_opt()
            .map(Self)
            .ok_or_else(|| {
                TimestampError::Overflow("DateTime<Local> value out of i64 nanosecond range".into())
            })
    }
}

impl From<NanoTimestamp> for DateTime<Utc> {
    fn from(ts: NanoTimestamp) -> Self {
        Utc.timestamp_nanos(ts.0)
    }
}

impl From<NanoTimestamp> for DateTime<Local> {
    fn from(ts: NanoTimestamp) -> Self {
        let utc: DateTime<Utc> = ts.into();
        utc.with_timezone(&Local)
    }
}

impl From<NanoTimestamp> for TimeDelta {
    fn from(ts: NanoTimestamp) -> Self {
        TimeDelta::nanoseconds(ts.0)
    }
}

impl Display for NanoDelta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Debug for NanoDelta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}ns", self)
    }
}

impl From<i64> for NanoDelta {
    fn from(nanos: i64) -> Self {
        Self(nanos)
    }
}

impl Add<TimeDelta> for NanoDelta {
    type Output = NanoDelta;

    fn add(self, rhs: TimeDelta) -> Self::Output {
        NanoDelta::from(
            self.0
                + rhs
                    .num_nanoseconds()
                    .expect("TimeDelta duration is too large to be represented as i64 nanoseconds and will overflow"),
        )
    }
}
impl Add<NanoDelta> for NanoDelta {
    type Output = NanoDelta;

    fn add(self, rhs: NanoDelta) -> Self::Output {
        NanoDelta::from(self.0 + rhs.0)
    }
}

impl Sub<TimeDelta> for NanoDelta {
    type Output = NanoDelta;

    fn sub(self, rhs: TimeDelta) -> Self::Output {
        NanoDelta::from(
            self.0
                - rhs
                    .num_nanoseconds()
                    .expect("TimeDelta duration is too large to be represented as i64 nanoseconds and will overflow"),
        )
    }
}
impl Sub<NanoDelta> for NanoDelta {
    type Output = NanoDelta;

    fn sub(self, rhs: NanoDelta) -> Self::Output {
        NanoDelta::from(self.0 - rhs.0)
    }
}

impl TryFrom<TimeDelta> for NanoDelta {
    type Error = TimestampError;

    fn try_from(delta: TimeDelta) -> Result<Self, Self::Error> {
        delta.num_nanoseconds().map(Self).ok_or_else(|| {
            TimestampError::Overflow("TimeDelta duration is too large to fit in NanoDelta".into())
        })
    }
}

impl From<NanoDelta> for TimeDelta {
    fn from(delta: NanoDelta) -> Self {
        TimeDelta::nanoseconds(delta.0)
    }
}

impl TryFrom<NanoDelta> for std::time::Duration {
    type Error = TimestampError;

    fn try_from(delta: NanoDelta) -> Result<Self, Self::Error> {
        if delta.0 < 0 {
            Err(TimestampError::Overflow(
                "NanoDelta duration is negative".into(),
            ))
        } else {
            Ok(std::time::Duration::from_nanos(delta.0 as u64))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::convert::TryInto;

    use super::*;

    #[test]
    fn nano_timestamp_creation() {
        let ts = NanoTimestamp::from(1000);
        assert_eq!(ts.0, 1000);
    }

    #[test]
    fn nano_delta_creation() {
        let delta = NanoDelta::from(1000);
        assert_eq!(delta.0, 1000);
    }

    #[test]
    fn nano_timestamp_from_datetime() {
        let ts_val: i64 = 1000;
        let ts = NanoTimestamp::from(ts_val);
        let dt_utc = DateTime::<Utc>::from(ts);
        let round_trip_ts: NanoTimestamp = dt_utc.try_into().unwrap();
        assert_eq!(round_trip_ts.as_nanos(), ts_val);

        // Test edge case for chrono::DateTime<Utc> to NanoTimestamp
        // The actual max for timestamp_nanos_opt is around year 2262
        // Create a DateTime<Utc> directly from i64::MAX nanoseconds
        let max_datetime_chrono = Utc.timestamp_nanos(i64::MAX);
        assert!(NanoTimestamp::try_from(max_datetime_chrono).is_ok());

        // Constructing a DateTime known to be out of i64 range for nanoseconds
        // is hard, as chrono itself might limit it before
        // timestamp_nanos_opt is called. The test for from_rfc3339
        // covers parsing strings that might result in this.
    }

    #[test]
    fn nano_delta_from_timedelta() {
        let td_val: i64 = 1000;
        let td = TimeDelta::nanoseconds(td_val);
        let nano_delta: NanoDelta = td.try_into().unwrap();
        assert_eq!(nano_delta.as_nanos(), td_val);

        // TimeDelta is in milliseconds, so it can represent
        // a much larger range than NanoDelta, which is in nanoseconds.
        assert!(NanoDelta::try_from(TimeDelta::MAX).is_err());
        assert!(NanoDelta::try_from(TimeDelta::MIN).is_err());
    }

    #[test]
    fn nano_timestamp_add() {
        let ts = NanoTimestamp::from(1000);
        let ts2 = ts + NanoDelta::from(2000);
        assert_eq!(ts2.0, 3000);
    }

    #[test]
    fn nano_timestamp_sub() {
        let ts = NanoTimestamp::from(1000);
        let ts2 = ts - NanoDelta::from(2000);
        assert_eq!(ts2.0, -1000);
    }

    #[test]
    fn nano_delta_add() {
        let delta = NanoDelta::from(1000);
        let delta2 = delta + NanoDelta::from(2000);
        assert_eq!(delta2.0, 3000);
    }

    #[test]
    fn nano_delta_sub() {
        let delta = NanoDelta::from(1000);
        let delta2 = delta - NanoDelta::from(2000);
        assert_eq!(delta2.0, -1000);
    }

    #[test]
    fn timestamp_overflow() {
        // Test overflow cases
        assert!(NanoTimestamp::from_hours(i64::MAX).is_err());
        assert!(NanoTimestamp::from_hours(i64::MIN).is_err());
        assert!(NanoTimestamp::from_minutes(i64::MAX).is_err());
        assert!(NanoTimestamp::from_minutes(i64::MIN).is_err());
        assert!(NanoTimestamp::from_secs(i64::MAX).is_err());
        assert!(NanoTimestamp::from_secs(i64::MIN).is_err());
        assert!(NanoTimestamp::from_millis(i64::MAX).is_err());
        assert!(NanoTimestamp::from_millis(i64::MIN).is_err());
        assert!(NanoTimestamp::from_micros(i64::MAX).is_err());
        assert!(NanoTimestamp::from_micros(i64::MIN).is_err());

        // Test valid cases
        assert!(NanoTimestamp::from_hours(24).is_ok());
        assert!(NanoTimestamp::from_minutes(60).is_ok());
        assert!(NanoTimestamp::from_secs(3600).is_ok());
    }

    #[test]
    fn timestamp_max() {
        let dt = DateTime::<Utc>::from(NanoTimestamp::from(i64::MAX));
        assert_eq!(dt.timestamp_nanos_opt().unwrap(), i64::MAX);
    }

    #[test]
    fn timestamp_overflow_from_datetime() {
        // This test demonstrated the unwrap previously. Now we test TryFrom.
        // Constructing a DateTime<Utc> that would cause timestamp_nanos_opt() to return
        // None is tricky because DateTime itself has limits.
        // We rely on i64::MAX for timestamp_nanos.
        let dt_max_nanos = Utc.timestamp_nanos(i64::MAX);
        let ts_result = NanoTimestamp::try_from(dt_max_nanos);
        assert!(ts_result.is_ok());
        assert_eq!(ts_result.unwrap().as_nanos(), i64::MAX);

        // A date far in the future that chrono can represent but might exceed i64
        // nanos. chrono::NaiveDate::MAX is year 262143. This will certainly be
        // None from timestamp_nanos_opt. However, creating such a DateTime<Utc>
        // is complex. The from_rfc3339 test with an out-of-range date is more
        // practical.

        // Test parsing of out-of-range date string
        let far_future_date_str = "+275760-09-13T00:00:00Z"; // Far beyond i64 nanos capacity
        match NanoTimestamp::from_rfc3339(far_future_date_str) {
            Err(TimestampError::Parse(_)) => { /* Expected */ }
            Err(e) => panic!("Expected Parse error, got {:?}", e),
            Ok(_) => panic!("Expected error for far future date"),
        }

        let far_past_date_str = "-271821-04-20T00:00:00Z"; // Far beyond i64 nanos capacity (negative)
        match NanoTimestamp::from_rfc3339(far_past_date_str) {
            Err(TimestampError::Parse(_)) => { /* Expected */ }
            Err(e) => panic!("Expected Parse error, got {:?}", e),
            Ok(_) => panic!("Expected error for far past date"),
        }

        // Test invalid format
        let invalid_date_str = "not a date";
        match NanoTimestamp::from_rfc3339(invalid_date_str) {
            Err(TimestampError::Parse(_)) => { /* Expected */ }
            Err(e) => panic!("Expected Parse error, got {:?}", e),
            Ok(_) => panic!("Expected error for invalid date string"),
        }
    }

    #[test]
    fn timestamp_conversion() {
        let ts = NanoTimestamp::from(1_123_456_789_000_000);
        assert_eq!(ts.as_nanos(), 1_123_456_789_000_000);
        assert_eq!(ts.as_micros(), 1_123_456_789_000);
        assert_eq!(ts.as_millis(), 1_123_456_789);
        assert_eq!(ts.as_secs(), 1_123_456);
        assert_eq!(ts.as_minutes(), 18724);
        assert_eq!(ts.as_hours(), 312);
        assert_eq!(ts.as_days(), 13);

        let delta = NanoDelta::from(1_123_456_789_000_000);
        assert_eq!(delta.as_nanos(), 1_123_456_789_000_000);
        assert_eq!(delta.as_micros(), 1_123_456_789_000);
        assert_eq!(delta.as_millis(), 1_123_456_789);
        assert_eq!(delta.as_secs(), 1_123_456);
        assert_eq!(delta.as_minutes(), 18724);
        assert_eq!(delta.as_hours(), 312);
        assert_eq!(delta.as_days(), 13);
    }

    #[test]
    fn timestamp_conversion_to_datetime_utc() {
        let ts = NanoTimestamp::from(1_123_456_789_000_000);
        let dt: DateTime<Utc> = ts.as_utc();
        assert_eq!(dt.timestamp_nanos_opt().unwrap(), 1_123_456_789_000_000);
        assert_eq!(dt.to_rfc2822(), "Wed, 14 Jan 1970 00:04:16 +0000");
        assert_eq!(dt.to_rfc3339(), "1970-01-14T00:04:16.789+00:00");
        let dt_paris = DateTime::<Utc>::from(dt).with_timezone(&chrono_tz::Europe::Paris);
        assert_eq!(dt_paris.to_rfc2822(), "Wed, 14 Jan 1970 01:04:16 +0100");
        assert_eq!(dt_paris.to_rfc3339(), "1970-01-14T01:04:16.789+01:00");
        let dt_newyork = DateTime::<Utc>::from(dt).with_timezone(&chrono_tz::America::New_York);
        assert_eq!(dt_newyork.to_rfc2822(), "Tue, 13 Jan 1970 19:04:16 -0500");
        assert_eq!(dt_newyork.to_rfc3339(), "1970-01-13T19:04:16.789-05:00");
    }

    #[test]
    fn timestamp_conversion_from_now() {
        let dt = chrono::Utc::now();
        println!("{:?}", dt);
        // This is expected to fail in year ~2262
        let nt = NanoTimestamp::try_from(dt).unwrap();
        println!("{:?}", nt);
    }
}
