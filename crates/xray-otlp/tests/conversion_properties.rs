use proptest::prelude::*;
use prost::Message;
use xray_core::AttributeValue;
use xray_otlp::decode_export_report;
use xray_otlp::opentelemetry::proto::{
    collector::trace::v1::ExportTraceServiceRequest,
    common::v1::{any_value::Value, AnyValue, KeyValue},
    trace::v1::{ResourceSpans, ScopeSpans, Span, Status},
};

proptest! {
    #[test]
    fn preserves_generated_span_fields(
        trace_id in any::<[u8; 16]>().prop_filter("nonzero trace ID", |id| *id != [0; 16]),
        span_id in any::<[u8; 8]>().prop_filter("nonzero span ID", |id| *id != [0; 8]),
        counts in any::<[u32; 3]>(),
        kind in any::<i32>(),
        status in any::<i32>(),
        values in prop::collection::vec(any::<i64>(), 2..8),
        schema_url in r"https://example\.test/scope/[a-z0-9]{1,16}",
    ) {
        let request = ExportTraceServiceRequest {
            resource_spans: vec![ResourceSpans {
                scope_spans: vec![ScopeSpans {
                    schema_url: schema_url.clone(),
                    spans: vec![Span {
                        trace_id: trace_id.to_vec(),
                        span_id: span_id.to_vec(),
                        kind,
                        status: Some(Status { code: status, message: "fixture".into() }),
                        dropped_attributes_count: counts[0],
                        dropped_events_count: counts[1],
                        dropped_links_count: counts[2],
                        attributes: values.iter().map(|value| KeyValue {
                            key: "duplicate".into(),
                            value: Some(AnyValue { value: Some(Value::IntValue(*value)) }),
                            ..KeyValue::default()
                        }).collect(),
                        ..Span::default()
                    }],
                    ..ScopeSpans::default()
                }],
                ..ResourceSpans::default()
            }],
        };

        let bytes = request.encode_to_vec();
        let report = decode_export_report(&bytes, bytes.len()).unwrap();
        prop_assert_eq!(report.rejected_spans, 0);
        let scope = &report.resource_spans[0].scope_spans[0];
        prop_assert_eq!(&scope.schema_url, &schema_url);
        let span = &scope.spans[0];
        prop_assert_eq!(span.trace_id.as_bytes(), &trace_id);
        prop_assert_eq!(span.span_id.as_bytes(), &span_id);
        prop_assert_eq!(span.kind.code(), kind);
        prop_assert_eq!(span.status.as_ref().unwrap().code.code(), status);
        prop_assert_eq!([span.dropped_attributes_count, span.dropped_events_count, span.dropped_links_count], counts);
        prop_assert_eq!(span.attributes.len(), values.len());
        for (attribute, value) in span.attributes.iter().zip(values) {
            prop_assert_eq!(&attribute.key, "duplicate");
            prop_assert!(matches!(attribute.value, Some(AttributeValue::Int(actual)) if actual == value));
        }
    }

    #[test]
    fn bounded_arbitrary_protobuf_never_panics(payload in prop::collection::vec(any::<u8>(), 0..512)) {
        let _ = decode_export_report(&payload, 512);
    }
}
