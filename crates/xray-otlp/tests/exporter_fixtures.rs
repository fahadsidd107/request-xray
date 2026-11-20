use prost::Message;
use xray_core::{AttributeValue, StatusCode};
use xray_otlp::opentelemetry::proto::collector::trace::v1::ExportTraceServiceRequest;
use xray_otlp::{decode_export, decode_export_report, DecodeError, IdField, SpanError, TimeField};

const SINGLE: &[u8] = include_bytes!("../../../tests/fixtures/otlp/single.pb");
const DISTRIBUTED: &[u8] = include_bytes!("../../../tests/fixtures/otlp/distributed.pb");
const LARGE: &[u8] = include_bytes!("../../../tests/fixtures/otlp/large-attribute.pb");

#[test]
fn official_exporter_single_span() {
    let report = decode_export_report(SINGLE, SINGLE.len()).unwrap();
    assert_eq!(report.rejected_spans, 0);
    assert_eq!(report.resource_spans.len(), 1);
    let spans = &report.resource_spans[0].scope_spans[0].spans;
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].name, "single");
    assert_eq!(spans[0].trace_id.as_bytes(), &[0x11; 16]);
    assert!(spans[0].parent_span_id.is_none());
    assert_eq!(
        spans[0].interval.start().as_i64(),
        1_793_500_000_000_000_001
    );
    assert_eq!(spans[0].interval.duration_ns(), 1);
}

#[test]
fn official_exporter_parallel_distributed_trace() {
    let groups = decode_export(DISTRIBUTED).unwrap();
    assert_eq!(groups.len(), 2);
    for (group, service) in groups.iter().zip(["orders-fixture", "payments-fixture"]) {
        let service_attribute = &group.resource.as_ref().unwrap().attributes[0];
        assert_eq!(service_attribute.key, "service.name");
        assert!(
            matches!(&service_attribute.value, Some(AttributeValue::String(value)) if value == service)
        );
    }

    let orders = &groups[0].scope_spans[0];
    let payments = &groups[1].scope_spans[0];
    assert_eq!(orders.schema_url, "https://example.test/scope/orders");
    assert_eq!(payments.schema_url, "https://example.test/scope/payments");
    assert_eq!(orders.spans.len(), 2);
    assert_eq!(payments.spans.len(), 1);
    let root = orders
        .spans
        .iter()
        .find(|span| span.name == "checkout")
        .unwrap();
    let inventory = orders
        .spans
        .iter()
        .find(|span| span.name == "inventory")
        .unwrap();
    let charge = &payments.spans[0];
    assert_eq!(charge.name, "charge");
    for child in [inventory, charge] {
        assert_eq!(child.trace_id, root.trace_id);
        assert_eq!(child.parent_span_id, Some(root.span_id));
        assert!(child.interval.start() > root.interval.start());
        assert!(child.interval.end() < root.interval.end());
    }
    assert!(inventory.interval.start() < charge.interval.start());
    assert!(charge.interval.start() < inventory.interval.end());

    assert_eq!(charge.dropped_attributes_count, 1);
    assert!(
        matches!(&charge.attributes.iter().find(|attribute| attribute.key == "city").unwrap().value,
        Some(AttributeValue::String(text)) if text == "Zürich 東京")
    );
    assert_eq!(charge.events[0].name, "paiement refusé");
    assert!(matches!(
        charge.events[0].attributes[0].value,
        Some(AttributeValue::Bool(false))
    ));
    assert_eq!(charge.links[0].trace_id, inventory.trace_id);
    assert_eq!(charge.links[0].span_id, inventory.span_id);
    assert_eq!(charge.status.as_ref().unwrap().code, StatusCode::ERROR);
    assert_eq!(charge.status.as_ref().unwrap().message, "synthetic decline");
}

#[test]
fn official_exporter_large_attribute_is_preserved_under_the_byte_limit() {
    let report = decode_export_report(LARGE, LARGE.len()).unwrap();
    let span = &report.resource_spans[0].scope_spans[0].spans[0];
    assert!(matches!(&span.attributes[0].value,
        Some(AttributeValue::String(text)) if text.len() == 64 * 1024 && text.bytes().all(|byte| byte == b'x')));

    let err = decode_export_report(LARGE, LARGE.len() - 1).err().unwrap();
    assert_eq!(err.code(), "resource_limit");
    assert!(
        matches!(err, DecodeError::ResourceLimit { max_payload_bytes } if max_payload_bytes == LARGE.len() - 1)
    );
}

#[test]
fn mutations_reject_invalid_spans_and_preserve_valid_siblings() {
    let mut request = ExportTraceServiceRequest::decode(DISTRIBUTED).unwrap();
    let spans = &mut request.resource_spans[0].scope_spans[0].spans;
    spans[0].trace_id.pop();
    // A span with two invalid fields must count as a single rejection.
    spans[0].end_time_unix_nano = u64::MAX;
    spans[1].start_time_unix_nano = u64::MAX;
    let payload = request.encode_to_vec();

    let err = decode_export(&payload).err().unwrap();
    assert_eq!(err.code(), "invalid_span");
    assert!(matches!(
        err,
        DecodeError::Span {
            resource_index: 0,
            scope_index: 0,
            span_index: 0,
            source: SpanError::Id {
                field: IdField::Trace,
                ..
            },
        }
    ));

    request.resource_spans[0].scope_spans[0].spans.remove(0);
    let err = decode_export(&request.encode_to_vec()).err().unwrap();
    assert!(matches!(
        err,
        DecodeError::Span {
            source: SpanError::Time {
                field: TimeField::Start,
                ..
            },
            ..
        }
    ));

    let report = decode_export_report(&payload, payload.len()).unwrap();
    assert_eq!(report.rejected_spans, 2);
    assert!(report.resource_spans[0].scope_spans[0].spans.is_empty());
    assert_eq!(
        report.resource_spans[1].scope_spans[0].spans[0].name,
        "charge"
    );

    let truncated = &DISTRIBUTED[..DISTRIBUTED.len() - 1];
    let err = decode_export_report(truncated, truncated.len())
        .err()
        .unwrap();
    assert_eq!(err.code(), "invalid_payload");
    let err = decode_export_report(truncated, 0).err().unwrap();
    assert_eq!(err.code(), "resource_limit");
}
