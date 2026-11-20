use std::{
    future::ready,
    io::Write,
    sync::{Arc, Mutex},
};

use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
    Router,
};
use flate2::{write::GzEncoder, Compression};
use prost::Message;
use tower::ServiceExt;
use xray_collector::{router, Config, Redaction, StorageUnavailable};
use xray_core::{AttributeValue, ResourceSpans};
use xray_otlp::opentelemetry::proto::{
    collector::trace::v1::{ExportTraceServiceRequest, ExportTraceServiceResponse},
    common::v1::{any_value::Value, AnyValue, KeyValue, KeyValueList},
};

const SINGLE: &[u8] = include_bytes!("../../../tests/fixtures/otlp/single.pb");
const DISTRIBUTED: &[u8] = include_bytes!("../../../tests/fixtures/otlp/distributed.pb");
const LARGE: &[u8] = include_bytes!("../../../tests/fixtures/otlp/large-attribute.pb");

type Captured = Arc<Mutex<Vec<Vec<ResourceSpans>>>>;

fn capture(config: Config) -> (Router, Captured) {
    let batches = Captured::default();
    let received = batches.clone();
    let app = router(config, move |batch| {
        received.lock().unwrap().push(batch);
        ready(Ok(()))
    });
    (app, batches)
}

fn request(payload: impl Into<Body>) -> Request<Body> {
    Request::post("/v1/traces")
        .header(header::CONTENT_TYPE, "application/x-protobuf")
        .body(payload.into())
        .unwrap()
}

async fn send(app: &Router, request: Request<Body>) -> (StatusCode, axum::body::Bytes) {
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "application/x-protobuf"
    );
    let (parts, body) = response.into_parts();
    (parts.status, to_bytes(body, 4096).await.unwrap())
}

#[derive(Clone, PartialEq, Message)]
struct RpcStatus {
    #[prost(string, tag = "2")]
    message: String,
}

async fn rejected(app: &Router, request: Request<Body>, expected: StatusCode, category: &str) {
    let (status, bytes) = send(app, request).await;
    assert_eq!(status, expected);
    let error = RpcStatus::decode(bytes).unwrap();
    assert!(error.message.starts_with(category), "{}", error.message);
}

#[tokio::test]
async fn binds_loopback_only() {
    assert_eq!(Config::default().listen.to_string(), "127.0.0.1:4318");
    for address in ["127.0.0.1:0", "[::1]:0"] {
        let config = Config {
            listen: address.parse().unwrap(),
            ..Config::default()
        };
        // IPv6 may be disabled by the host, so test rejection specifically below.
        if address.starts_with("127") {
            assert!(config
                .bind()
                .await
                .unwrap()
                .local_addr()
                .unwrap()
                .ip()
                .is_loopback());
        } else {
            match config.bind().await {
                Ok(listener) => assert!(listener.local_addr().unwrap().ip().is_loopback()),
                Err(error) => assert_ne!(error.kind(), std::io::ErrorKind::InvalidInput),
            }
        }
    }
    for address in ["0.0.0.0:0", "[::]:0", "192.0.2.1:0"] {
        let config = Config {
            listen: address.parse().unwrap(),
            ..Config::default()
        };
        assert_eq!(
            config.bind().await.unwrap_err().kind(),
            std::io::ErrorKind::InvalidInput
        );
    }
}

#[tokio::test]
async fn responds_with_full_partial_and_total_rejection() {
    let (app, batches) = capture(Config::default());
    let (status, bytes) = send(&app, request(SINGLE)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(ExportTraceServiceResponse::decode(bytes)
        .unwrap()
        .partial_success
        .is_none());
    let (status, bytes) = send(&app, request(Body::empty())).await;
    assert_eq!(status, StatusCode::OK);
    assert!(ExportTraceServiceResponse::decode(bytes)
        .unwrap()
        .partial_success
        .is_none());

    let mut input = ExportTraceServiceRequest::decode(DISTRIBUTED).unwrap();
    input.resource_spans[0].scope_spans[0].spans[0]
        .span_id
        .clear();
    let (status, bytes) = send(&app, request(input.encode_to_vec())).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        ExportTraceServiceResponse::decode(bytes)
            .unwrap()
            .partial_success
            .unwrap()
            .rejected_spans,
        1
    );
    assert_eq!(batches.lock().unwrap().len(), 2);
    assert_eq!(batches.lock().unwrap()[1][0].scope_spans[0].spans.len(), 1);

    let mut input = ExportTraceServiceRequest::decode(SINGLE).unwrap();
    input.resource_spans[0].scope_spans[0].spans[0]
        .span_id
        .clear();
    rejected(
        &app,
        request(input.encode_to_vec()),
        StatusCode::BAD_REQUEST,
        "invalid_span",
    )
    .await;
    assert_eq!(batches.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn rejects_bad_requests_and_does_not_acknowledge_an_unavailable_sink() {
    let app = router(Config::default(), |_| ready(Err(StorageUnavailable)));
    rejected(
        &app,
        request(SINGLE),
        StatusCode::SERVICE_UNAVAILABLE,
        "storage_unavailable",
    )
    .await;
    rejected(
        &app,
        request(&b"\x0a\x01"[..]),
        StatusCode::BAD_REQUEST,
        "invalid_payload",
    )
    .await;
    let mut wrong_type = request(SINGLE);
    wrong_type
        .headers_mut()
        .insert(header::CONTENT_TYPE, "application/json".parse().unwrap());
    rejected(
        &app,
        wrong_type,
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        "unsupported_content_type",
    )
    .await;
    let mut ambiguous = request(SINGLE);
    ambiguous.headers_mut().append(
        header::CONTENT_TYPE,
        "application/x-protobuf".parse().unwrap(),
    );
    rejected(
        &app,
        ambiguous,
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        "unsupported_content_type",
    )
    .await;
    let mut wrong_encoding = request(SINGLE);
    wrong_encoding
        .headers_mut()
        .insert(header::CONTENT_ENCODING, "gzip, br".parse().unwrap());
    rejected(
        &app,
        wrong_encoding,
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        "unsupported_content_encoding",
    )
    .await;
    let mut method = request(SINGLE);
    *method.method_mut() = axum::http::Method::GET;
    rejected(
        &app,
        method,
        StatusCode::METHOD_NOT_ALLOWED,
        "method_not_allowed",
    )
    .await;
}

fn gzip(payload: &[u8]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(payload).unwrap();
    encoder.finish().unwrap()
}

fn gzipped(payload: Vec<u8>) -> Request<Body> {
    let mut req = request(payload);
    req.headers_mut()
        .insert(header::CONTENT_ENCODING, "gzip".parse().unwrap());
    req
}

#[tokio::test]
async fn enforces_wire_and_decompressed_byte_caps() {
    let compressed = gzip(SINGLE);
    let mut config = Config::default();
    config.limits.compressed_bytes = compressed.len();
    config.limits.decoded_bytes = SINGLE.len();
    let (app, batches) = capture(config);
    assert_eq!(
        send(&app, gzipped(compressed.clone())).await.0,
        StatusCode::OK
    );
    assert_eq!(send(&app, request(SINGLE)).await.0, StatusCode::OK);
    let mut too_long = compressed.clone();
    too_long.push(1);
    rejected(
        &app,
        gzipped(too_long),
        StatusCode::PAYLOAD_TOO_LARGE,
        "resource_limit",
    )
    .await;
    rejected(
        &app,
        request(DISTRIBUTED),
        StatusCode::PAYLOAD_TOO_LARGE,
        "resource_limit",
    )
    .await;
    rejected(
        &app,
        gzipped(gzip(&[0; 16 * 1024])),
        StatusCode::PAYLOAD_TOO_LARGE,
        "resource_limit",
    )
    .await;

    let mut truncated = compressed;
    truncated.pop();
    rejected(
        &app,
        gzipped(truncated),
        StatusCode::BAD_REQUEST,
        "invalid_payload",
    )
    .await;
    rejected(
        &app,
        gzipped(vec![]),
        StatusCode::BAD_REQUEST,
        "invalid_payload",
    )
    .await;
    assert_eq!(batches.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn bounded_shape_includes_nested_attributes_and_invalid_spans() {
    for limits in [
        xray_collector::Limits {
            protobuf_fields: 1,
            ..Default::default()
        },
        xray_collector::Limits {
            spans: 2,
            ..Default::default()
        },
        xray_collector::Limits {
            events_per_span: 0,
            ..Default::default()
        },
        xray_collector::Limits {
            links_per_span: 0,
            ..Default::default()
        },
        xray_collector::Limits {
            attributes_per_list: 1,
            ..Default::default()
        },
    ] {
        let (app, batches) = capture(Config {
            limits,
            ..Config::default()
        });
        rejected(
            &app,
            request(DISTRIBUTED),
            StatusCode::PAYLOAD_TOO_LARGE,
            "resource_limit",
        )
        .await;
        assert!(batches.lock().unwrap().is_empty());
    }
    let (app, batches) = capture(Config::default());
    rejected(
        &app,
        request(LARGE),
        StatusCode::PAYLOAD_TOO_LARGE,
        "resource_limit",
    )
    .await;
    assert!(batches.lock().unwrap().is_empty());

    let mut input = ExportTraceServiceRequest::decode(SINGLE).unwrap();
    let span = &mut input.resource_spans[0].scope_spans[0].spans[0];
    span.attributes = vec![attribute(
        "nested",
        Value::KvlistValue(KeyValueList {
            values: vec![string("value", "inner")],
        }),
    )];
    let config = Config {
        limits: xray_collector::Limits {
            attribute_depth: 0,
            ..Default::default()
        },
        ..Config::default()
    };
    let (app, _) = capture(config);
    rejected(
        &app,
        request(input.encode_to_vec()),
        StatusCode::PAYLOAD_TOO_LARGE,
        "resource_limit",
    )
    .await;
}

fn attribute(key: &str, value: Value) -> KeyValue {
    KeyValue {
        key: key.into(),
        value: Some(AnyValue { value: Some(value) }),
        ..Default::default()
    }
}

fn string(key: &str, value: &str) -> KeyValue {
    attribute(key, Value::StringValue(value.into()))
}

#[tokio::test]
async fn passes_only_redacted_data_to_sink() {
    let mut input = ExportTraceServiceRequest::decode(DISTRIBUTED).unwrap();
    let group = &mut input.resource_spans[1];
    group
        .resource
        .as_mut()
        .unwrap()
        .attributes
        .push(string("API_KEY", "secret"));
    let scope = &mut group.scope_spans[0];
    scope
        .scope
        .as_mut()
        .unwrap()
        .attributes
        .push(string("customer.email", "secret"));
    let charge = &mut scope.spans[0];
    charge.name = "GET /pay?token=secret".into();
    charge.status.as_mut().unwrap().message = "secret".into();
    charge.attributes.extend([
        string(
            "url.full",
            "https://alice:secret@example.test/pay?q=secret#secret",
        ),
        string("unlisted", "secret"),
        attribute(
            "nested",
            Value::KvlistValue(KeyValueList {
                values: vec![
                    string("accessToken", "secret"),
                    string("http.target", "//alice:secret@example.test/pay?secret"),
                ],
            }),
        ),
    ]);
    charge.events[0].attributes.extend([
        string("http.response.header.set-cookie", "secret"),
        string("value", "keep"),
    ]);
    charge.links[0]
        .attributes
        .push(string("Authorization", "secret"));

    let config = Config {
        redaction: Redaction {
            allow: Some(
                [
                    "service.name",
                    "url.full",
                    "nested",
                    "http.target",
                    "customer.email",
                    "accessToken",
                    "API_KEY",
                    "value",
                    "Authorization",
                ]
                .map(str::to_owned)
                .to_vec(),
            ),
            deny: vec!["CUSTOMER.EMAIL".into()],
        },
        ..Config::default()
    };
    let (app, batches) = capture(config);
    assert_eq!(
        send(&app, request(input.encode_to_vec())).await.0,
        StatusCode::OK
    );
    let batches = batches.lock().unwrap();
    let group = &batches[0][1];
    let resource = group.resource.as_ref().unwrap();
    assert_eq!(resource.attributes.len(), 1);
    assert_eq!(resource.attributes[0].key, "service.name");
    let scope = &group.scope_spans[0];
    assert!(scope.scope.as_ref().unwrap().attributes.is_empty());
    let charge = &scope.spans[0];
    assert_eq!(charge.name, "GET /pay");
    assert!(charge.status.as_ref().unwrap().message.is_empty());
    assert_eq!(charge.attributes.len(), 2);
    assert!(matches!(&charge.attributes[0].value,
        Some(AttributeValue::String(url)) if url == "https://example.test/pay"));
    assert!(matches!(&charge.attributes[1].value,
        Some(AttributeValue::KeyValueList(entries)) if entries.len() == 1 &&
        matches!(&entries[0].value, Some(AttributeValue::String(url)) if url == "//example.test/pay")));
    assert_eq!(charge.events[0].attributes.len(), 1);
    assert_eq!(charge.events[0].attributes[0].key, "value");
    assert!(charge.links[0].attributes.is_empty());
    assert_eq!(
        charge.dropped_attributes_count, 1,
        "keep the exporter-reported count"
    );
}

#[tokio::test]
async fn rejects_over_capacity_before_reading_and_acknowledges_only_after_commit() {
    use axum::body::HttpBody;
    use std::{
        convert::Infallible,
        pin::Pin,
        task::{Context, Poll},
        time::Duration,
    };
    use tokio::{sync::Notify, time::timeout};
    use xray_collector::{Admission, Ingestion, SinkOutcome};

    struct UnreadBody;

    impl HttpBody for UnreadBody {
        type Data = axum::body::Bytes;
        type Error = Infallible;

        fn poll_frame(
            self: Pin<&mut Self>,
            _: &mut Context<'_>,
        ) -> Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
            panic!("rejected request body must not be read")
        }
    }

    let ingestion = Ingestion::new(Config {
        admission: Admission {
            max_in_flight: 1,
            ..Admission::default()
        },
        ..Config::default()
    });
    let started = Arc::new(Notify::new());
    let commit = Arc::new(Notify::new());
    let app = ingestion.router({
        let started = started.clone();
        let commit = commit.clone();
        move |_| {
            let started = started.clone();
            let commit = commit.clone();
            async move {
                started.notify_one();
                commit.notified().await;
                Ok(SinkOutcome::Committed {
                    duplicate_spans: 1,
                    conflicting_spans: 0,
                })
            }
        }
    });

    let first = tokio::spawn(app.clone().oneshot(request(SINGLE)));
    timeout(Duration::from_secs(2), started.notified())
        .await
        .unwrap();
    assert!(!first.is_finished());
    assert_eq!(ingestion.diagnostics().accepted_spans, 0);

    let response = app
        .clone()
        .oneshot(request(Body::new(UnreadBody)))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(response.headers()[header::RETRY_AFTER], "1");
    assert_eq!(ingestion.diagnostics().admission_rejections, 1);

    commit.notify_one();
    assert_eq!(first.await.unwrap().unwrap().status(), StatusCode::OK);
    assert_eq!(ingestion.diagnostics().accepted_spans, 1);
    assert_eq!(ingestion.diagnostics().duplicate_spans, 1);

    let response = send(&app, request(Body::empty())).await;
    assert_eq!(
        response.0,
        StatusCode::OK,
        "permit must be released after the commit"
    );
    ingestion.begin_shutdown();
    rejected(
        &app,
        request(Body::new(UnreadBody)),
        StatusCode::SERVICE_UNAVAILABLE,
        "collector_shutting_down",
    )
    .await;
    assert_eq!(ingestion.diagnostics().shutdown_rejections, 1);
}

#[tokio::test]
async fn body_and_sink_timeouts_release_capacity_without_acknowledgment() {
    use std::{future::pending, time::Duration};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use xray_collector::{Admission, Ingestion};

    let ingestion = Ingestion::new(Config {
        admission: Admission {
            max_in_flight: 1,
            body_timeout: Duration::from_millis(50),
            sink_timeout: Duration::from_millis(50),
        },
        ..Config::default()
    });

    let app = ingestion.router(|_| pending());
    rejected(
        &app,
        request(SINGLE),
        StatusCode::SERVICE_UNAVAILABLE,
        "storage_unavailable: write timeout",
    )
    .await;
    assert_eq!(ingestion.diagnostics().write_timeouts, 1);
    assert_eq!(ingestion.diagnostics().write_failures, 1);
    assert_eq!(ingestion.diagnostics().accepted_spans, 0);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (stop, shutdown) = tokio::sync::oneshot::channel();
    let server = tokio::spawn({
        let ingestion = ingestion.clone();
        async move {
            ingestion
                .serve(
                    listener,
                    |_| async { panic!("incomplete body reached sink") },
                    async {
                        let _ = shutdown.await;
                    },
                )
                .await
        }
    });
    let mut connection = tokio::net::TcpStream::connect(address).await.unwrap();
    connection.write_all(b"POST /v1/traces HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/x-protobuf\r\nContent-Length: 1\r\nConnection: close\r\n\r\n").await.unwrap();
    let mut reply = Vec::new();
    tokio::time::timeout(Duration::from_secs(2), connection.read_to_end(&mut reply))
        .await
        .unwrap()
        .unwrap();
    assert!(reply.starts_with(b"HTTP/1.1 408 "));
    assert_eq!(ingestion.diagnostics().body_timeouts, 1);
    assert_eq!(send(&app, request(Body::empty())).await.0, StatusCode::OK);
    stop.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(2), server)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn shutdown_drains_admitted_commit_on_a_live_connection() {
    use std::time::Duration;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        sync::Notify,
        time::timeout,
    };
    use xray_collector::{Ingestion, SinkOutcome};

    let ingestion = Ingestion::new(Config::default());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let started = Arc::new(Notify::new());
    let stopping = Arc::new(Notify::new());
    let commit = Arc::new(Notify::new());
    let (stop, shutdown) = tokio::sync::oneshot::channel();
    let server = tokio::spawn({
        let ingestion = ingestion.clone();
        let started = started.clone();
        let stopping = stopping.clone();
        let commit = commit.clone();
        async move {
            ingestion
                .clone()
                .serve(
                    listener,
                    move |_| {
                        let started = started.clone();
                        let commit = commit.clone();
                        async move {
                            started.notify_one();
                            commit.notified().await;
                            Ok(SinkOutcome::Committed {
                                duplicate_spans: 0,
                                conflicting_spans: 0,
                            })
                        }
                    },
                    async move {
                        shutdown.await.unwrap();
                        ingestion.begin_shutdown();
                        stopping.notify_one();
                    },
                )
                .await
        }
    });

    let mut connection = tokio::net::TcpStream::connect(address).await.unwrap();
    connection.write_all(format!(
        "POST /v1/traces HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/x-protobuf\r\nContent-Length: {}\r\n\r\n", SINGLE.len()
    ).as_bytes()).await.unwrap();
    connection.write_all(SINGLE).await.unwrap();
    timeout(Duration::from_secs(2), started.notified())
        .await
        .unwrap();

    stop.send(()).unwrap();
    timeout(Duration::from_secs(2), stopping.notified())
        .await
        .unwrap();
    assert!(!server.is_finished());
    assert_eq!(ingestion.diagnostics().accepted_spans, 0);
    commit.notify_one();

    let mut reply = Vec::new();
    timeout(Duration::from_secs(2), connection.read_to_end(&mut reply))
        .await
        .unwrap()
        .unwrap();
    assert!(reply.starts_with(b"HTTP/1.1 200 "));
    timeout(Duration::from_secs(2), server)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(ingestion.diagnostics().accepted_spans, 1);
    assert!(tokio::net::TcpStream::connect(address).await.is_err());
}

#[tokio::test]
async fn diagnostics_distinguish_validation_rejections_and_redaction() {
    use xray_collector::{Ingestion, SinkOutcome};

    let ingestion = Ingestion::new(Config::default());
    let app = ingestion.router(|_| ready(Ok(SinkOutcome::ValidationOnly)));
    let mut input = ExportTraceServiceRequest::decode(DISTRIBUTED).unwrap();
    input.resource_spans[0].scope_spans[0].spans[0]
        .trace_id
        .clear();
    let (status, body) = send(&app, request(input.encode_to_vec())).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        ExportTraceServiceResponse::decode(body)
            .unwrap()
            .partial_success
            .unwrap()
            .rejected_spans,
        1
    );
    rejected(
        &app,
        request(LARGE),
        StatusCode::PAYLOAD_TOO_LARGE,
        "resource_limit",
    )
    .await;
    rejected(
        &app,
        request(&b"\x0a\x01"[..]),
        StatusCode::BAD_REQUEST,
        "invalid_payload",
    )
    .await;

    let stats = ingestion.diagnostics();
    assert_eq!(stats.accepted_spans, 0);
    assert_eq!(stats.validation_only_spans, 2);
    assert_eq!(stats.rejected_spans, 1);
    assert_eq!(stats.redacted_spans, 1);
    assert_eq!(stats.redacted_fields, 1);
    assert_eq!(stats.invalid_payloads, 1);
    assert_eq!(stats.size_errors, 1);
    assert_eq!(stats.write_failures, 0);
    assert_eq!(stats.duplicate_spans, 0);
}
