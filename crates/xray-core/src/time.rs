use serde::{Serialize, Serializer};
use std::{error::Error, fmt};

/// Nanoseconds since the Unix epoch, within the signed SQLite integer range.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct UnixNanos(i64);

impl UnixNanos {
    pub const fn as_i64(self) -> i64 {
        self.0
    }
}

impl TryFrom<u64> for UnixNanos {
    type Error = TimestampOutOfRange;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        i64::try_from(value)
            .map(Self)
            .map_err(|_| TimestampOutOfRange)
    }
}

impl TryFrom<i64> for UnixNanos {
    type Error = TimestampOutOfRange;

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        if value < 0 {
            return Err(TimestampOutOfRange);
        }
        Ok(Self(value))
    }
}

impl fmt::Display for UnixNanos {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl Serialize for UnixNanos {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimestampOutOfRange;

impl fmt::Display for TimestampOutOfRange {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("timestamp is outside the supported Unix nanosecond range")
    }
}

impl Error for TimestampOutOfRange {}

/// A validated span interval. Equal start and end times are valid.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpanInterval {
    start: UnixNanos,
    end: UnixNanos,
}

impl SpanInterval {
    pub fn new(start: UnixNanos, end: UnixNanos) -> Result<Self, EndBeforeStart> {
        if end < start {
            return Err(EndBeforeStart);
        }
        Ok(Self { start, end })
    }

    pub const fn start(self) -> UnixNanos {
        self.start
    }

    pub const fn end(self) -> UnixNanos {
        self.end
    }

    /// Elapsed wall time in nanoseconds. The validated range makes subtraction safe.
    pub const fn duration_ns(self) -> i64 {
        self.end.0 - self.start.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EndBeforeStart;

impl fmt::Display for EndBeforeStart {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("span end precedes its start")
    }
}

impl Error for EndBeforeStart {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamp_range_is_checked() {
        assert_eq!(UnixNanos::try_from(0_u64).unwrap().as_i64(), 0);
        assert_eq!(
            UnixNanos::try_from(i64::MAX as u64).unwrap().as_i64(),
            i64::MAX
        );
        assert_eq!(
            UnixNanos::try_from(i64::MAX as u64 + 1),
            Err(TimestampOutOfRange)
        );
        assert_eq!(UnixNanos::try_from(u64::MAX), Err(TimestampOutOfRange));
        assert_eq!(UnixNanos::try_from(-1_i64), Err(TimestampOutOfRange));
    }

    #[test]
    fn validates_interval_without_overflow() {
        let epoch = UnixNanos::try_from(0_i64).unwrap();
        let max = UnixNanos::try_from(i64::MAX).unwrap();
        assert_eq!(
            SpanInterval::new(epoch, max).unwrap().duration_ns(),
            i64::MAX
        );
        assert_eq!(SpanInterval::new(max, max).unwrap().duration_ns(), 0);
        assert_eq!(SpanInterval::new(max, epoch), Err(EndBeforeStart));
    }

    #[test]
    fn json_uses_decimal_strings() {
        let timestamp = UnixNanos::try_from(9_007_199_254_740_993_i64).unwrap();
        assert_eq!(
            serde_json::to_string(&timestamp).unwrap(),
            "\"9007199254740993\""
        );
    }
}
