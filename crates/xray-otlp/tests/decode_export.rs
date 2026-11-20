use prost::Message;
use xray_core::{AttributeValue, InvalidId};
use xray_otlp::opentelemetry::proto::{
    collector::trace::v1::ExportTraceServiceRequest,
    common::v1::{
        any_value::Value, AnyValue, ArrayValue, EntityRef, InstrumentationScope, KeyValue,
        KeyValueList,
    },
    resource::v1::Resource,
    trace::v1::{
        span::{Event, Link},
        ResourceSpans, ScopeSpans, Span, Status,
    },
};
use xray_otlp::{convert_span, decode_export, DecodeError, IdField, SpanError, TimeField};

const START: u64 = 9_007_199_254_740_993;

fn span(id: u8) -> Span {
    Span {
        trace_id: vec![1; 16],
        span_id: vec![id; 8],
        name: "checkout".into(),
        start_time_unix_nano: START,
        end_time_unix_nano: START + 1,
        ..Span::default()
    }
}

fn attribute(key: &str, value: Value) -> KeyValue {
    KeyValue {
        key: key.into(),
        value: Some(AnyValue { value: Some(value) }),
        ..KeyValue::default()
    }
}

fn request(spans: Vec<Span>) -> ExportTraceServiceRequest {
    ExportTraceServiceRequest {
        resource_spans: vec![ResourceSpans {
            resource: Some(Resource {
                attributes: vec![attribute(
                    "service.name",
                    Value::StringValue("orders".into()),
                )],
                dropped_attributes_count: 1,
                entity_refs: vec![EntityRef {
                    schema_url: "entity-schema".into(),
                    r#type: "service".into(),
                    id_keys: vec!["service.name".into()],
                    description_keys: vec![],
                }],
            }),
            schema_url: "resource-schema".into(),
            scope_spans: vec![ScopeSpans {
                scope: Some(InstrumentationScope {
                    name: "instrumentation".into(),
                    version: "1.2".into(),
                    attributes: vec![attribute("enabled", Value::BoolValue(true))],
                    dropped_attributes_count: 2,
                }),
                schema_url: "scope-schema".into(),
                spans,
            }],
        }],
    }
}

#[test]
fn decodes_hierarchy_without_losing_trace_fields() {
    let root = span(2);
    let child = Span {
        parent_span_id: root.span_id.clone(),
        trace_state: "sample=1".into(),
        flags: 0x8000_0301,
        kind: 17,
        status: Some(Status {
            code: 9,
            message: "status detail".into(),
        }),
        attributes: vec![
            attribute(
                "détail",
                Value::ArrayValue(ArrayValue {
                    values: vec![
                        AnyValue {
                            value: Some(Value::IntValue(i64::MIN)),
                        },
                        AnyValue {
                            value: Some(Value::KvlistValue(KeyValueList {
                                values: vec![attribute("bytes", Value::BytesValue(vec![0, 255]))],
                            })),
                        },
                        AnyValue {
                            value: Some(Value::DoubleValue(-0.0)),
                        },
                    ],
                }),
            ),
            KeyValue {
                key: "détail".into(),
                value: None,
                key_strindex: 0,
            },
            KeyValue {
                key: "empty".into(),
                value: Some(AnyValue::default()),
                key_strindex: 0,
            },
        ],
        dropped_attributes_count: 3,
        dropped_events_count: 4,
        dropped_links_count: 5,
        events: vec![Event {
            time_unix_nano: START,
            name: "paiement".into(),
            attributes: vec![attribute("attempt", Value::IntValue(1))],
            dropped_attributes_count: 6,
        }],
        links: vec![Link {
            trace_id: vec![7; 16],
            span_id: vec![8; 8],
            trace_state: "sample=2".into(),
            flags: 0x4000_0101,
            attributes: vec![attribute("async", Value::BoolValue(true))],
            dropped_attributes_count: 7,
        }],
        ..span(3)
    };
    let mut input = request(vec![root, child]);
    input.resource_spans.push(ResourceSpans {
        scope_spans: vec![ScopeSpans::default()],
        ..ResourceSpans::default()
    });
    let decoded = decode_export(&input.encode_to_vec()).unwrap();

    assert_eq!(decoded.len(), 2);
    assert!(decoded[1].resource.is_none());
    assert!(decoded[1].scope_spans[0].scope.is_none());
    assert!(decoded[1].scope_spans[0].spans.is_empty());
    let group = &decoded[0];
    assert_eq!(group.schema_url, "resource-schema");
    let resource = group.resource.as_ref().unwrap();
    assert_eq!(resource.dropped_attributes_count, 1);
    assert_eq!(resource.entity_refs[0].entity_type, "service");
    assert_eq!(resource.entity_refs[0].id_keys, ["service.name"]);
    assert!(
        matches!(&resource.attributes[0].value, Some(AttributeValue::String(value)) if value == "orders")
    );
    assert_eq!(group.scope_spans[0].schema_url, "scope-schema");
    let scope = group.scope_spans[0].scope.as_ref().unwrap();
    assert_eq!(
        (
            &*scope.name,
            &*scope.version,
            scope.dropped_attributes_count
        ),
        ("instrumentation", "1.2", 2)
    );
    assert!(matches!(
        scope.attributes[0].value,
        Some(AttributeValue::Bool(true))
    ));

    let spans = &group.scope_spans[0].spans;
    assert_eq!(spans.len(), 2);
    assert!(spans[0].parent_span_id.is_none());
    assert!(spans[0].status.is_none());
    let child = &spans[1];
    assert_eq!(child.trace_id, spans[0].trace_id);
    assert_eq!(child.parent_span_id, Some(spans[0].span_id));
    assert_eq!(child.span_id.as_bytes(), &[3; 8]);
    assert_eq!(child.interval.start().as_i64(), START as i64);
    assert_eq!(child.interval.duration_ns(), 1);
    assert_eq!(
        (&*child.trace_state, child.flags, child.kind.code()),
        ("sample=1", 0x8000_0301, 17)
    );
    let status = child.status.as_ref().unwrap();
    assert_eq!((status.code.code(), &*status.message), (9, "status detail"));
    assert_eq!(
        [
            child.dropped_attributes_count,
            child.dropped_events_count,
            child.dropped_links_count
        ],
        [3, 4, 5]
    );

    assert_eq!(child.attributes[0].key, "détail");
    assert_eq!(child.attributes[1].key, "détail");
    assert!(child.attributes[1].value.is_none());
    assert!(matches!(
        child.attributes[2].value,
        Some(AttributeValue::Empty)
    ));
    let Some(AttributeValue::Array(values)) = &child.attributes[0].value else {
        panic!("missing nested array");
    };
    assert!(matches!(values[0], AttributeValue::Int(i64::MIN)));
    assert!(
        matches!(&values[1], AttributeValue::KeyValueList(items) if matches!(&items[0].value, Some(AttributeValue::Bytes(bytes)) if bytes == &[0, 255]))
    );
    assert!(
        matches!(values[2], AttributeValue::Double(value) if value.to_bits() == (-0.0_f64).to_bits())
    );

    let event = &child.events[0];
    assert_eq!(
        (
            &*event.name,
            event.time_unix_nano.as_i64(),
            event.dropped_attributes_count
        ),
        ("paiement", START as i64, 6)
    );
    assert!(matches!(
        event.attributes[0].value,
        Some(AttributeValue::Int(1))
    ));
    let link = &child.links[0];
    assert_eq!(link.trace_id.as_bytes(), &[7; 16]);
    assert_eq!(link.span_id.as_bytes(), &[8; 8]);
    assert_eq!(
        (
            &*link.trace_state,
            link.flags,
            link.dropped_attributes_count
        ),
        ("sample=2", 0x4000_0101, 7)
    );
    assert!(matches!(
        link.attributes[0].value,
        Some(AttributeValue::Bool(true))
    ));
}

#[test]
fn distinguishes_bad_protobuf_from_an_invalid_span() {
    assert!(matches!(
        decode_export(&[0x0a, 0x01]),
        Err(DecodeError::Protobuf(_))
    ));
    assert!(decode_export(&[]).unwrap().is_empty());

    let valid = span(2);
    let invalid = Span {
        parent_span_id: vec![0; 8],
        ..span(3)
    };
    let payload = request(vec![valid, invalid]).encode_to_vec();
    assert!(matches!(
        decode_export(&payload),
        Err(DecodeError::Span {
            resource_index: 0,
            scope_index: 0,
            span_index: 1,
            source: SpanError::Id {
                field: IdField::Parent,
                source: InvalidId::AllZero
            },
        })
    ));
}

#[test]
fn rejects_invalid_link_ids_and_timestamps() {
    let invalid_link = Span {
        links: vec![Link {
            trace_id: vec![7; 16],
            span_id: vec![],
            ..Link::default()
        }],
        ..span(2)
    };
    assert!(matches!(
        convert_span(invalid_link),
        Err(SpanError::Id {
            field: IdField::LinkSpan(0),
            ..
        })
    ));

    let overflow = Span {
        end_time_unix_nano: u64::MAX,
        ..span(2)
    };
    assert!(matches!(
        convert_span(overflow),
        Err(SpanError::Time {
            field: TimeField::End,
            ..
        })
    ));
    let backwards = Span {
        end_time_unix_nano: START - 1,
        ..span(2)
    };
    assert!(matches!(
        convert_span(backwards),
        Err(SpanError::EndBeforeStart(_))
    ));
    let invalid_event = Span {
        events: vec![Event {
            time_unix_nano: u64::MAX,
            ..Event::default()
        }],
        ..span(2)
    };
    assert!(matches!(
        convert_span(invalid_event),
        Err(SpanError::Time {
            field: TimeField::Event(0),
            ..
        })
    ));
}
