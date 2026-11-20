use std::{path::Path, process::Stdio, time::Duration};

use prost::Message;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
    process::{Child, Command},
};
use xray_core::{AttributeValue, StatusCode, TraceId};
use xray_otlp::opentelemetry::proto::{
    collector::trace::v1::{
        ExportTracePartialSuccess, ExportTraceServiceRequest, ExportTraceServiceResponse,
    },
    common::v1::{any_value::Value, AnyValue, KeyValue},
};
use xray_storage::Repository;

const DISTRIBUTED: &[u8] = include_bytes!("../../../tests/fixtures/otlp/distributed.pb");

async fn start(db: &Path) -> (Child, String) {
    let mut collector = Command::new(env!("CARGO_BIN_EXE_xray-collector"))
        .env("REQUEST_XRAY_OTLP_LISTEN", "127.0.0.1:0")
        .env("REQUEST_XRAY_DB_PATH", db)
        .env("REQUEST_XRAY_RETENTION_AGE_SECONDS", "604800")
        .env("REQUEST_XRAY_RETENTION_MAX_BYTES", "524288000")
        .env("REQUEST_XRAY_BODY_TIMEOUT_MS", "2000")
        .env("REQUEST_XRAY_SINK_TIMEOUT_MS", "2000")
        .env("REQUEST_XRAY_MAX_IN_FLIGHT", "8")
        .env_remove("REQUEST_XRAY_MAX_COMPRESSED_BYTES")
        .env_remove("REQUEST_XRAY_MAX_DECODED_BYTES")
        .env_remove("REQUEST_XRAY_ALLOW_ATTRIBUTES")
        .env_remove("REQUEST_XRAY_DENY_ATTRIBUTES")
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut line = String::new();
    let mut stderr = BufReader::new(collector.stderr.as_mut().unwrap());
    tokio::time::timeout(Duration::from_secs(10), stderr.read_line(&mut line))
        .await
        .unwrap()
        .unwrap();
    let address = line
        .strip_prefix("OTLP/HTTP listener at http://")
        .unwrap_or_else(|| panic!("{line}"))
        .split('/')
        .next()
        .unwrap()
        .to_owned();
    (collector, address)
}

async fn export(address: &str, payload: &[u8]) -> (u16, Vec<u8>) {
    tokio::time::timeout(Duration::from_secs(5), async {
        let mut stream = TcpStream::connect(address).await.unwrap();
        stream.write_all(format!(
            "POST /v1/traces HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/x-protobuf\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            payload.len(),
        ).as_bytes()).await.unwrap();
        stream.write_all(payload).await.unwrap();
        let mut response = Vec::new();
        stream.read_to_end(&mut response).await.unwrap();
        let split = response.windows(4).position(|bytes| bytes == b"\r\n\r\n").unwrap();
        let status = std::str::from_utf8(&response[..split]).unwrap().split_whitespace().nth(1).unwrap().parse().unwrap();
        (status, response[split + 4..].to_vec())
    }).await.unwrap()
}

fn attr(key: &str, text: &str) -> KeyValue {
    KeyValue {
        key: key.into(),
        value: Some(AnyValue {
            value: Some(Value::StringValue(text.into())),
        }),
        ..Default::default()
    }
}

#[tokio::test]
async fn collector_commits_exporter_fixture_and_recovers_on_restart() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("traces.sqlite");
    let trace_id = TraceId::try_from([0x11; 16]).unwrap();
    let mut first = ExportTraceServiceRequest::decode(DISTRIBUTED).unwrap();
    let root = first.resource_spans[0].scope_spans[0].spans.pop().unwrap();
    assert_eq!(root.name, "checkout");
    let charge = &mut first.resource_spans[1].scope_spans[0].spans[0];
    charge.attributes.extend([
        attr("Authorization", "fixture-secret"),
        attr(
            "url.full",
            "https://user:fixture-secret@example.test/pay?token=fixture-secret",
        ),
    ]);
    let mut late = first.clone();
    late.resource_spans.truncate(1);
    late.resource_spans[0].scope_spans[0].spans = vec![root];
    let original = first.encode_to_vec();

    let (mut collector, address) = start(&db).await;
    let (status, response) = export(&address, &original).await;
    assert_eq!(status, 200);
    assert!(ExportTraceServiceResponse::decode(response.as_slice())
        .unwrap()
        .partial_success
        .is_none());
    collector.kill().await.unwrap(); // Simulate process loss just after acknowledgment.

    let (mut collector, address) = start(&db).await;
    let repo = Repository::open(&db).await.unwrap();
    let before = repo.get_trace(trace_id).await.unwrap().unwrap();
    assert_eq!(before.span_count, 2);
    assert_eq!(before.missing_parent_count, 2);
    assert!(before.has_error);
    assert_eq!(export(&address, &original).await.0, 200);

    first.resource_spans[1].scope_spans[0].spans[0].name = "conflicting change".into();
    let (status, response) = export(&address, &first.encode_to_vec()).await;
    assert_eq!(status, 200);
    assert!(matches!(
        ExportTraceServiceResponse::decode(response.as_slice())
            .unwrap()
            .partial_success,
        Some(ExportTracePartialSuccess {
            rejected_spans: 1,
            ..
        })
    ));
    assert_eq!(export(&address, &late.encode_to_vec()).await.0, 200);
    assert_eq!(
        export(&address, &DISTRIBUTED[..DISTRIBUTED.len() - 1])
            .await
            .0,
        400
    );

    let after = repo.get_trace(trace_id).await.unwrap().unwrap();
    assert_eq!(after.span_count, 3);
    assert_eq!(after.missing_parent_count, 0);
    assert_eq!(after.first_seen_ns, before.first_seen_ns);
    assert!(after.start_ns < before.start_ns);
    let spans = repo.list_spans(trace_id, None, 10).await.unwrap();
    assert_eq!(
        spans
            .iter()
            .map(|stored| stored.span.name.as_str())
            .collect::<Vec<_>>(),
        ["checkout", "inventory", "charge"]
    );
    let charge = &spans[2].span;
    assert_eq!(charge.status.as_ref().unwrap().code, StatusCode::ERROR);
    assert_eq!(charge.events[0].name, "paiement refusé");
    assert_eq!(charge.links[0].span_id, spans[1].span.span_id);
    assert!(charge
        .attributes
        .iter()
        .all(|attribute| attribute.key != "Authorization"));
    assert!(charge.attributes.iter().any(|attribute| attribute.key == "url.full"
        && matches!(&attribute.value, Some(AttributeValue::String(url)) if url == "https://example.test/pay")));
    assert!(charge.attributes.iter().any(|attribute| attribute.key == "city"
        && matches!(&attribute.value, Some(AttributeValue::String(city)) if city == "Zürich 東京")));
    collector.kill().await.unwrap();
    repo.close().await;
}
