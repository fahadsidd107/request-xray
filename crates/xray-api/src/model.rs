use base64::{engine::general_purpose::STANDARD, Engine};
use serde::Serialize;
use xray_core::{Attribute, AttributeValue, Resource, Scope, Span};
use xray_storage::{StorageStats, StoredSpan, TraceStatus, TraceSummary};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Trace {
    trace_id: String,
    started_at_unix_ns: String,
    ended_at_unix_ns: String,
    duration_ns: String,
    first_seen_at_unix_ns: String,
    span_count: i64,
    service_count: i64,
    has_error: bool,
    status: &'static str,
    completeness: Completeness,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Completeness {
    state: &'static str,
    missing_parent_count: i64,
    last_seen_at_unix_ns: String,
}

impl From<TraceSummary> for Trace {
    fn from(row: TraceSummary) -> Self {
        let state = if row.has_missing_parents() {
            "missing_parents"
        } else {
            "observed"
        };
        Self {
            trace_id: row.trace_id.to_string(),
            started_at_unix_ns: row.start_ns.to_string(),
            ended_at_unix_ns: row.end_ns.to_string(),
            duration_ns: (row.end_ns - row.start_ns).to_string(),
            first_seen_at_unix_ns: row.first_seen_ns.to_string(),
            span_count: row.span_count,
            service_count: row.service_count,
            has_error: row.has_error,
            status: match row.status {
                TraceStatus::Error => "error",
                TraceStatus::Ok => "ok",
                TraceStatus::Unset => "unset",
            },
            completeness: Completeness {
                state,
                missing_parent_count: row.missing_parent_count,
                last_seen_at_unix_ns: row.last_seen_ns.to_string(),
            },
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpanItem {
    trace_id: String,
    span_id: String,
    parent_span_id: Option<String>,
    name: String,
    kind: i32,
    trace_state: String,
    flags: u32,
    started_at_unix_ns: String,
    ended_at_unix_ns: String,
    duration_ns: String,
    status: Option<Status>,
    route: Option<String>,
    resource_schema_url: String,
    resource: Option<ResourceItem>,
    scope_schema_url: String,
    scope: Option<ScopeItem>,
    attributes: Vec<AttributeItem>,
    events: Vec<Event>,
    links: Vec<Link>,
    dropped_attributes_count: u32,
    dropped_events_count: u32,
    dropped_links_count: u32,
}

#[derive(Serialize)]
struct Status {
    code: i32,
    message: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ResourceItem {
    attributes: Vec<AttributeItem>,
    dropped_attributes_count: u32,
    entity_refs: Vec<Entity>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Entity {
    schema_url: String,
    entity_type: String,
    id_keys: Vec<String>,
    description_keys: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ScopeItem {
    name: String,
    version: String,
    attributes: Vec<AttributeItem>,
    dropped_attributes_count: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Event {
    name: String,
    time_unix_ns: String,
    attributes: Vec<AttributeItem>,
    dropped_attributes_count: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Link {
    trace_id: String,
    span_id: String,
    trace_state: String,
    flags: u32,
    attributes: Vec<AttributeItem>,
    dropped_attributes_count: u32,
}

#[derive(Serialize)]
struct AttributeItem {
    key: String,
    value: Option<Value>,
}

#[derive(Serialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
enum Value {
    Empty,
    String(String),
    Bool(bool),
    Int(String),
    Double(String),
    Bytes(String),
    Array(Vec<Value>),
    KeyValueList(Vec<AttributeItem>),
}

fn attributes(values: Vec<Attribute>) -> Vec<AttributeItem> {
    values
        .into_iter()
        .map(|entry| AttributeItem {
            key: entry.key,
            value: entry.value.map(value),
        })
        .collect()
}

fn value(input: AttributeValue) -> Value {
    match input {
        AttributeValue::Empty => Value::Empty,
        AttributeValue::String(text) => Value::String(text),
        AttributeValue::Bool(boolean) => Value::Bool(boolean),
        AttributeValue::Int(integer) => Value::Int(integer.to_string()),
        AttributeValue::Double(double) => Value::Double(double.to_string()),
        AttributeValue::Bytes(bytes) => Value::Bytes(STANDARD.encode(bytes)),
        AttributeValue::Array(items) => Value::Array(items.into_iter().map(value).collect()),
        AttributeValue::KeyValueList(items) => Value::KeyValueList(attributes(items)),
    }
}

fn resource(input: Resource) -> ResourceItem {
    ResourceItem {
        attributes: attributes(input.attributes),
        dropped_attributes_count: input.dropped_attributes_count,
        entity_refs: input
            .entity_refs
            .into_iter()
            .map(|entity| Entity {
                schema_url: entity.schema_url,
                entity_type: entity.entity_type,
                id_keys: entity.id_keys,
                description_keys: entity.description_keys,
            })
            .collect(),
    }
}

fn scope(input: Scope) -> ScopeItem {
    ScopeItem {
        name: input.name,
        version: input.version,
        attributes: attributes(input.attributes),
        dropped_attributes_count: input.dropped_attributes_count,
    }
}

impl From<StoredSpan> for SpanItem {
    fn from(row: StoredSpan) -> Self {
        let Span {
            trace_id,
            span_id,
            parent_span_id,
            trace_state,
            flags,
            name,
            kind,
            interval,
            attributes: span_attrs,
            dropped_attributes_count,
            events,
            dropped_events_count,
            links,
            dropped_links_count,
            status,
        } = row.span;
        let route = span_attrs
            .iter()
            .find_map(|attribute| match &attribute.value {
                Some(AttributeValue::String(route))
                    if attribute.key == "http.route" && !route.is_empty() =>
                {
                    Some(route.clone())
                }
                _ => None,
            });
        Self {
            trace_id: trace_id.to_string(),
            span_id: span_id.to_string(),
            parent_span_id: parent_span_id.map(|id| id.to_string()),
            name,
            kind: kind.code(),
            trace_state,
            flags,
            started_at_unix_ns: interval.start().to_string(),
            ended_at_unix_ns: interval.end().to_string(),
            duration_ns: interval.duration_ns().to_string(),
            status: status.map(|status| Status {
                code: status.code.code(),
                message: (!status.message.is_empty()).then_some(status.message),
            }),
            route,
            resource_schema_url: row.resource_schema_url,
            resource: row.resource.map(resource),
            scope_schema_url: row.scope_schema_url,
            scope: row.scope.map(scope),
            attributes: attributes(span_attrs),
            dropped_attributes_count,
            dropped_events_count,
            dropped_links_count,
            events: events
                .into_iter()
                .map(|event| Event {
                    name: event.name,
                    time_unix_ns: event.time_unix_nano.to_string(),
                    attributes: attributes(event.attributes),
                    dropped_attributes_count: event.dropped_attributes_count,
                })
                .collect(),
            links: links
                .into_iter()
                .map(|link| Link {
                    trace_id: link.trace_id.to_string(),
                    span_id: link.span_id.to_string(),
                    trace_state: link.trace_state,
                    flags: link.flags,
                    attributes: attributes(link.attributes),
                    dropped_attributes_count: link.dropped_attributes_count,
                })
                .collect(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub runtime: Option<RuntimeStats>,
    pub trace_count: i64,
    pub span_count: i64,
    pub service_count: i64,
    pub error_trace_count: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStats {
    pub ingestion: IngestionStats,
    pub retention: RetentionStats,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IngestionStats {
    pub source_endpoint: &'static str,
    pub accepted_spans: u64,
    pub rejected_spans: u64,
    pub duplicate_spans: u64,
    pub conflicting_spans: u64,
    pub redacted_spans: u64,
    pub redacted_fields: u64,
    pub validation_only_spans: u64,
    pub size_errors: u64,
    pub invalid_payloads: u64,
    pub admission_rejections: u64,
    pub shutdown_rejections: u64,
    pub body_timeouts: u64,
    pub write_failures: u64,
    pub write_timeouts: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RetentionStats {
    pub deleted_traces: u64,
    pub live_bytes_at_last_pass: u64,
    pub failures: u64,
}

impl From<StorageStats> for Stats {
    fn from(row: StorageStats) -> Self {
        Self {
            runtime: None,
            trace_count: row.trace_count,
            span_count: row.span_count,
            service_count: row.service_count,
            error_trace_count: row.error_trace_count,
        }
    }
}
