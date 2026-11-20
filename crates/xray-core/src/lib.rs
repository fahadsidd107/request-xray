//! Canonical, transport-independent types for trace telemetry.

mod id;
mod time;

pub use id::{InvalidId, SpanId, TraceId};
pub use time::{EndBeforeStart, SpanInterval, TimestampOutOfRange, UnixNanos};

/// Span kind code. Unrecognized codes are preserved for forward compatibility.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpanKind(i32);

impl SpanKind {
    pub const UNSPECIFIED: Self = Self(0);
    pub const INTERNAL: Self = Self(1);
    pub const SERVER: Self = Self(2);
    pub const CLIENT: Self = Self(3);
    pub const PRODUCER: Self = Self(4);
    pub const CONSUMER: Self = Self(5);

    pub const fn from_code(code: i32) -> Self {
        Self(code)
    }

    pub const fn code(self) -> i32 {
        self.0
    }
}

/// Span status code. Unrecognized codes are preserved for forward compatibility.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StatusCode(i32);

impl StatusCode {
    pub const UNSET: Self = Self(0);
    pub const OK: Self = Self(1);
    pub const ERROR: Self = Self(2);

    pub const fn from_code(code: i32) -> Self {
        Self(code)
    }

    pub const fn code(self) -> i32 {
        self.0
    }
}

#[derive(Clone, PartialEq)]
pub struct SpanStatus {
    pub code: StatusCode,
    pub message: String,
}

/// Ordered attributes retain duplicate keys for ingestion-time validation.
pub type Attributes = Vec<Attribute>;

#[derive(Clone, PartialEq)]
pub struct Attribute {
    pub key: String,
    pub value: Option<AttributeValue>,
}

#[derive(Clone, PartialEq)]
pub enum AttributeValue {
    Empty,
    String(String),
    Bool(bool),
    Int(i64),
    Double(f64),
    Bytes(Vec<u8>),
    Array(Vec<Self>),
    KeyValueList(Attributes),
}

#[derive(Clone, PartialEq)]
pub struct Resource {
    pub attributes: Attributes,
    pub dropped_attributes_count: u32,
    pub entity_refs: Vec<EntityRef>,
}

#[derive(Clone, PartialEq)]
pub struct EntityRef {
    pub schema_url: String,
    pub entity_type: String,
    pub id_keys: Vec<String>,
    pub description_keys: Vec<String>,
}

#[derive(Clone, PartialEq)]
pub struct Scope {
    pub name: String,
    pub version: String,
    pub attributes: Attributes,
    pub dropped_attributes_count: u32,
}

#[derive(Clone, PartialEq)]
pub struct Event {
    pub time_unix_nano: UnixNanos,
    pub name: String,
    pub attributes: Attributes,
    pub dropped_attributes_count: u32,
}

#[derive(Clone, PartialEq)]
pub struct Link {
    pub trace_id: TraceId,
    pub span_id: SpanId,
    pub trace_state: String,
    pub attributes: Attributes,
    pub dropped_attributes_count: u32,
    pub flags: u32,
}

#[derive(Clone, PartialEq)]
pub struct Span {
    pub trace_id: TraceId,
    pub span_id: SpanId,
    pub parent_span_id: Option<SpanId>,
    pub trace_state: String,
    pub flags: u32,
    pub name: String,
    pub kind: SpanKind,
    pub interval: SpanInterval,
    pub attributes: Attributes,
    pub dropped_attributes_count: u32,
    pub events: Vec<Event>,
    pub dropped_events_count: u32,
    pub links: Vec<Link>,
    pub dropped_links_count: u32,
    pub status: Option<SpanStatus>,
}

#[derive(Clone, PartialEq)]
pub struct ResourceSpans {
    pub resource: Option<Resource>,
    pub schema_url: String,
    pub scope_spans: Vec<ScopeSpans>,
}

#[derive(Clone, PartialEq)]
pub struct ScopeSpans {
    pub scope: Option<Scope>,
    pub schema_url: String,
    pub spans: Vec<Span>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_and_status_codes_round_trip() {
        for code in [i32::MIN, -1, 0, 1, 2, 3, 4, 5, i32::MAX] {
            assert_eq!(SpanKind::from_code(code).code(), code);
            assert_eq!(StatusCode::from_code(code).code(), code);
        }
        assert_eq!(SpanKind::CLIENT.code(), 3);
        assert_eq!(StatusCode::ERROR.code(), 2);
    }
}
