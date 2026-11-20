use std::{error::Error, fmt};

use prost::Message;
use xray_core as core;

use crate::opentelemetry::proto::{
    collector::trace::v1::ExportTraceServiceRequest, common::v1 as common, resource::v1::Resource,
    trace::v1 as trace,
};

/// Decode an already size-limited protobuf request and convert its trace hierarchy.
///
/// A malformed protobuf or invalid span rejects the conversion. No partial batch is
/// returned. Use [`convert_span`] when per-span validation is needed.
pub fn decode_export(payload: &[u8]) -> Result<Vec<core::ResourceSpans>, DecodeError> {
    convert_export(decode_request(payload)?, Err)
}

/// Decode a size-limited request and tally every invalid span once.
///
/// Invalid spans are omitted. Resource and scope groups are retained in their
/// original order, including groups whose spans were all rejected.
pub fn decode_export_report(
    payload: &[u8],
    max_payload_bytes: usize,
) -> Result<DecodeReport, DecodeError> {
    if payload.len() > max_payload_bytes {
        return Err(DecodeError::ResourceLimit { max_payload_bytes });
    }

    Ok(convert_export_report(decode_request(payload)?))
}

/// Convert a decoded request after the caller has checked its structural limits.
pub fn convert_export_report(request: ExportTraceServiceRequest) -> DecodeReport {
    let mut rejected_spans = 0;
    let resource_spans = convert_export(request, |_| {
        rejected_spans += 1;
        Ok(())
    })
    .expect("the rejection callback always succeeds");
    DecodeReport {
        resource_spans,
        rejected_spans,
    }
}

pub struct DecodeReport {
    pub resource_spans: Vec<core::ResourceSpans>,
    pub rejected_spans: usize,
}

fn decode_request(payload: &[u8]) -> Result<ExportTraceServiceRequest, DecodeError> {
    ExportTraceServiceRequest::decode(payload).map_err(DecodeError::Protobuf)
}

fn convert_export(
    request: ExportTraceServiceRequest,
    mut reject: impl FnMut(DecodeError) -> Result<(), DecodeError>,
) -> Result<Vec<core::ResourceSpans>, DecodeError> {
    let mut resources = Vec::with_capacity(request.resource_spans.len());
    for (resource_index, group) in request.resource_spans.into_iter().enumerate() {
        let mut scope_spans = Vec::with_capacity(group.scope_spans.len());
        for (scope_index, group) in group.scope_spans.into_iter().enumerate() {
            let mut spans = Vec::new();
            for (span_index, span) in group.spans.into_iter().enumerate() {
                match convert_span(span) {
                    Ok(span) => spans.push(span),
                    Err(source) => reject(DecodeError::Span {
                        resource_index,
                        scope_index,
                        span_index,
                        source,
                    })?,
                }
            }
            scope_spans.push(core::ScopeSpans {
                scope: group.scope.map(convert_scope),
                schema_url: group.schema_url,
                spans,
            });
        }
        resources.push(core::ResourceSpans {
            resource: group.resource.map(convert_resource),
            schema_url: group.schema_url,
            scope_spans,
        });
    }
    Ok(resources)
}

/// Convert a decoded span without copying owned attribute payloads.
pub fn convert_span(span: trace::Span) -> Result<core::Span, SpanError> {
    let trace_id = parse_id(&span.trace_id, IdField::Trace)?;
    let span_id = parse_id(&span.span_id, IdField::Span)?;
    let parent_span_id = if span.parent_span_id.is_empty() {
        None
    } else {
        Some(parse_id(&span.parent_span_id, IdField::Parent)?)
    };

    let start = parse_time(span.start_time_unix_nano, TimeField::Start)?;
    let end = parse_time(span.end_time_unix_nano, TimeField::End)?;
    let interval = core::SpanInterval::new(start, end).map_err(SpanError::EndBeforeStart)?;
    let events = span
        .events
        .into_iter()
        .enumerate()
        .map(|(index, event)| {
            Ok(core::Event {
                time_unix_nano: parse_time(event.time_unix_nano, TimeField::Event(index))?,
                name: event.name,
                attributes: convert_attributes(event.attributes),
                dropped_attributes_count: event.dropped_attributes_count,
            })
        })
        .collect::<Result<_, _>>()?;
    let links = span
        .links
        .into_iter()
        .enumerate()
        .map(|(index, link)| {
            Ok(core::Link {
                trace_id: parse_id(&link.trace_id, IdField::LinkTrace(index))?,
                span_id: parse_id(&link.span_id, IdField::LinkSpan(index))?,
                trace_state: link.trace_state,
                flags: link.flags,
                attributes: convert_attributes(link.attributes),
                dropped_attributes_count: link.dropped_attributes_count,
            })
        })
        .collect::<Result<_, _>>()?;

    Ok(core::Span {
        trace_id,
        span_id,
        parent_span_id,
        trace_state: span.trace_state,
        flags: span.flags,
        name: span.name,
        kind: core::SpanKind::from_code(span.kind),
        interval,
        attributes: convert_attributes(span.attributes),
        dropped_attributes_count: span.dropped_attributes_count,
        events,
        dropped_events_count: span.dropped_events_count,
        links,
        dropped_links_count: span.dropped_links_count,
        status: span.status.map(|status| core::SpanStatus {
            code: core::StatusCode::from_code(status.code),
            message: status.message,
        }),
    })
}

fn parse_id<'a, T: TryFrom<&'a [u8], Error = core::InvalidId>>(
    bytes: &'a [u8],
    field: IdField,
) -> Result<T, SpanError> {
    T::try_from(bytes).map_err(|source| SpanError::Id { field, source })
}

fn parse_time(raw: u64, field: TimeField) -> Result<core::UnixNanos, SpanError> {
    core::UnixNanos::try_from(raw).map_err(|source| SpanError::Time { field, source })
}

fn convert_scope(scope: common::InstrumentationScope) -> core::Scope {
    core::Scope {
        name: scope.name,
        version: scope.version,
        attributes: convert_attributes(scope.attributes),
        dropped_attributes_count: scope.dropped_attributes_count,
    }
}

fn convert_resource(resource: Resource) -> core::Resource {
    core::Resource {
        attributes: convert_attributes(resource.attributes),
        dropped_attributes_count: resource.dropped_attributes_count,
        entity_refs: resource
            .entity_refs
            .into_iter()
            .map(|entity| core::EntityRef {
                schema_url: entity.schema_url,
                entity_type: entity.r#type,
                id_keys: entity.id_keys,
                description_keys: entity.description_keys,
            })
            .collect(),
    }
}

fn convert_attributes(attributes: Vec<common::KeyValue>) -> core::Attributes {
    attributes
        .into_iter()
        .map(|attribute| core::Attribute {
            // key_strindex is a Profiles dictionary reference.
            key: attribute.key,
            value: attribute.value.map(convert_value),
        })
        .collect()
}

fn convert_value(value: common::AnyValue) -> core::AttributeValue {
    use common::any_value::Value;
    use core::AttributeValue;

    match value.value {
        None | Some(Value::StringValueStrindex(_)) => AttributeValue::Empty,
        Some(Value::StringValue(value)) => AttributeValue::String(value),
        Some(Value::BoolValue(value)) => AttributeValue::Bool(value),
        Some(Value::IntValue(value)) => AttributeValue::Int(value),
        Some(Value::DoubleValue(value)) => AttributeValue::Double(value),
        Some(Value::BytesValue(value)) => AttributeValue::Bytes(value),
        Some(Value::ArrayValue(array)) => {
            AttributeValue::Array(array.values.into_iter().map(convert_value).collect())
        }
        Some(Value::KvlistValue(list)) => {
            AttributeValue::KeyValueList(convert_attributes(list.values))
        }
    }
}

#[derive(Debug)]
pub enum DecodeError {
    Protobuf(prost::DecodeError),
    ResourceLimit {
        max_payload_bytes: usize,
    },
    Span {
        resource_index: usize,
        scope_index: usize,
        span_index: usize,
        source: SpanError,
    },
}

impl DecodeError {
    /// Stable category for diagnostics.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Protobuf(_) => "invalid_payload",
            Self::ResourceLimit { .. } => "resource_limit",
            Self::Span { .. } => "invalid_span",
        }
    }
}

impl fmt::Display for DecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Protobuf(_) => formatter.write_str("invalid OTLP protobuf request"),
            Self::ResourceLimit { max_payload_bytes } => {
                write!(formatter, "OTLP protobuf request exceeds {max_payload_bytes} bytes")
            }
            Self::Span { resource_index, scope_index, span_index, source } => write!(
                formatter,
                "invalid span at resource {resource_index}, scope {scope_index}, span {span_index}: {source}"
            ),
        }
    }
}

impl Error for DecodeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Protobuf(source) => Some(source),
            Self::ResourceLimit { .. } => None,
            Self::Span { source, .. } => Some(source),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdField {
    Trace,
    Span,
    Parent,
    LinkTrace(usize),
    LinkSpan(usize),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimeField {
    Start,
    End,
    Event(usize),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpanError {
    Id {
        field: IdField,
        source: core::InvalidId,
    },
    Time {
        field: TimeField,
        source: core::TimestampOutOfRange,
    },
    EndBeforeStart(core::EndBeforeStart),
}

impl fmt::Display for SpanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Id { field, source } => write!(formatter, "{field:?} ID: {source}"),
            Self::Time { field, source } => write!(formatter, "{field:?} timestamp: {source}"),
            Self::EndBeforeStart(source) => source.fmt(formatter),
        }
    }
}

impl Error for SpanError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Id { source, .. } => Some(source),
            Self::Time { source, .. } => Some(source),
            Self::EndBeforeStart(source) => Some(source),
        }
    }
}
