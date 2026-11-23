use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
    Router,
};
use serde_json::{json, Value};
use tempfile::TempDir;
use tower::ServiceExt;
use xray_api::{bind, router, DEFAULT_LISTEN};
use xray_core::{
    Attribute, AttributeValue, SpanInterval, SpanStatus, StatusCode as SpanCode, TraceId, UnixNanos,
};
use xray_otlp::decode_export;
use xray_storage::Repository;

const ID: &str = "11111111111111111111111111111111";
const DISTRIBUTED: &[u8] = include_bytes!("../../../tests/fixtures/otlp/distributed.pb");
const SINGLE: &[u8] = include_bytes!("../../../tests/fixtures/otlp/single.pb");
const HOST: &str = "127.0.0.1:4731";

fn request(path: &str) -> Request<Body> {
    Request::get(path)
        .header(header::HOST, HOST)
        .body(Body::empty())
        .unwrap()
}

async fn send(app: &Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.headers()[header::CONTENT_TYPE], "application/json");
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert!(!response
        .headers()
        .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN));
    let (parts, body) = response.into_parts();
    let bytes = to_bytes(body, 16 * 1024 * 1024).await.unwrap();
    (parts.status, serde_json::from_slice(&bytes).unwrap())
}

async fn get(app: &Router, path: &str) -> (StatusCode, Value) {
    send(app, request(path)).await
}

async fn error(app: &Router, path: &str, status: StatusCode, code: &str) {
    let (actual, body) = get(app, path).await;
    assert_eq!(actual, status);
    assert_eq!(body["error"]["code"], code);
    assert!(body["error"]["requestId"]
        .as_str()
        .unwrap()
        .starts_with("q-"));
    assert!(body["error"]["message"].is_string());
}

async fn setup() -> (TempDir, Repository, Router) {
    let dir = tempfile::tempdir().unwrap();
    let repo = Repository::open(dir.path().join("query.sqlite"))
        .await
        .unwrap();
    let app = router(repo.clone(), DEFAULT_LISTEN).unwrap();
    (dir, repo, app)
}

fn trace_id(byte: u8) -> TraceId {
    TraceId::try_from([byte; 16]).unwrap()
}

async fn add_single(repo: &Repository, id: u8, start: i64, service: Option<&str>) {
    let mut groups = decode_export(SINGLE).unwrap();
    if let Some(service) = service {
        groups[0].resource.as_mut().unwrap().attributes[0].value =
            Some(AttributeValue::String(service.into()));
    }
    let span = &mut groups[0].scope_spans[0].spans[0];
    span.trace_id = trace_id(id);
    span.interval = SpanInterval::new(
        UnixNanos::try_from(start).unwrap(),
        UnixNanos::try_from(start + 1).unwrap(),
    )
    .unwrap();
    if id == 3 {
        span.status = Some(SpanStatus {
            code: SpanCode::OK,
            message: String::new(),
        });
    }
    repo.write_batch(groups).await.unwrap();
}

#[tokio::test]
async fn route_contract_and_persisted_precision() {
    let (_dir, repo, app) = setup().await;
    let schema: Value =
        serde_json::from_str(include_str!("../../../schemas/collector.openapi.yaml")).unwrap();
    let mut groups = decode_export(DISTRIBUTED).unwrap();
    let root = groups[0].scope_spans[0].spans.pop().unwrap();
    let charge = &mut groups[1].scope_spans[0].spans[0];
    charge.attributes.extend([
        Attribute {
            key: "http.route".into(),
            value: Some(AttributeValue::String("/orders/{id}".into())),
        },
        Attribute {
            key: "exact".into(),
            value: Some(AttributeValue::Int(i64::MAX)),
        },
    ]);
    repo.write_batch(groups).await.unwrap();
    let detail = format!("/api/v1/traces/{ID}");
    let spans = format!("{detail}/spans");
    let stages = [
        ("/api/v1/health", "/api/v1/health", "Health"),
        ("/api/v1/traces", "/api/v1/traces", "TracePage"),
        (detail.as_str(), "/api/v1/traces/{id}", "Trace"),
        (spans.as_str(), "/api/v1/traces/{id}/spans", "SpanPage"),
        ("/api/v1/services", "/api/v1/services", "ServicePage"),
        ("/api/v1/stats", "/api/v1/stats", "Stats"),
        (
            "/api/v1/config/capabilities",
            "/api/v1/config/capabilities",
            "Capabilities",
        ),
    ];
    for (path, template, name) in stages {
        let (status, body) = get(&app, path).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            schema["paths"][template]["get"]["responses"]["200"]["content"]["application/json"]
                ["schema"]["$ref"],
            format!("#/components/schemas/{name}")
        );
        assert_eq!(
            body.as_object()
                .unwrap()
                .keys()
                .cloned()
                .collect::<std::collections::BTreeSet<_>>(),
            serde_json::from_value::<std::collections::BTreeSet<String>>(
                schema["components"]["schemas"][name]["required"].clone()
            )
            .unwrap()
        );
    }
    assert_eq!(
        get(&app, "/api/v1/health").await.1,
        json!({"status":"ok","version":"0.1.0"})
    );
    assert_eq!(
        get(&app, "/api/v1/stats").await.1,
        json!({"traceCount":1,"spanCount":2,"serviceCount":2,"errorTraceCount":1,"runtime":null})
    );
    let capabilities = get(&app, "/api/v1/config/capabilities").await.1;
    assert_eq!(capabilities["ingestionCounters"], false);
    assert_eq!(capabilities["samplingCoverage"], false);
    let summary = get(&app, &detail).await.1;
    assert_eq!(summary["completeness"]["state"], "missing_parents");
    assert_eq!(summary["completeness"]["missingParentCount"], 2);
    assert!(summary["completeness"]["lastSeenAtUnixNs"]
        .as_str()
        .unwrap()
        .parse::<i64>()
        .is_ok());
    assert_eq!(summary["spanCount"], 2);
    assert_eq!(summary["status"], "error");
    let (status, first) = get(&app, &format!("{spans}?limit=1")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first["items"][0]["name"], "inventory");
    let (_, last) = get(
        &app,
        &format!("{spans}?cursor={}", first["nextCursor"].as_str().unwrap()),
    )
    .await;
    let charge = &last["items"][0];
    assert_eq!(charge["durationNs"], "30");
    assert_eq!(charge["startedAtUnixNs"], "1793500000000000030");
    assert_eq!(charge["route"], "/orders/{id}");
    assert_eq!(charge["events"][0]["timeUnixNs"], "1793500000000000040");
    assert_eq!(charge["links"][0]["spanId"], first["items"][0]["spanId"]);
    assert!(charge["attributes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry
            == &json!({"key":"exact","value":{"type":"int","value":"9223372036854775807"}})));
    assert_eq!(last["nextCursor"], Value::Null);
    let (status, filters) = get(&app, &format!("/api/v1/traces?traceId={ID}&service=payments-fixture&route=%2Forders%2F%7Bid%7D&error=true&status=error&sinceNs=1793500000000000020&untilNs=1793500000000000021&durationMinNs=40&durationMaxNs=40")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(filters["items"].as_array().unwrap().len(), 1);

    let mut late = decode_export(SINGLE).unwrap();
    late[0].scope_spans[0].spans = vec![root];
    repo.write_batch(late).await.unwrap();
    let updated = get(&app, &detail).await.1;
    assert_eq!(updated["spanCount"], 3);
    assert_eq!(updated["completeness"]["missingParentCount"], 0);
    assert_eq!(updated["completeness"]["state"], "observed");
    assert_eq!(
        get(&app, &spans).await.1["items"][0]["parentSpanId"],
        Value::Null
    );
}

#[tokio::test]
async fn keyset_pages_survive_an_interleaved_newer_insert() {
    let (_dir, repo, app) = setup().await;
    add_single(&repo, 1, 10, Some("alpha")).await;
    add_single(&repo, 2, 10, Some("beta")).await;
    let (_, first) = get(&app, "/api/v1/traces?limit=1").await;
    assert_eq!(first["items"][0]["traceId"], trace_id(2).to_string());
    add_single(&repo, 3, 20, Some("gamma")).await;
    let (_, second) = get(
        &app,
        &format!(
            "/api/v1/traces?limit=1&cursor={}",
            first["nextCursor"].as_str().unwrap()
        ),
    )
    .await;
    assert_eq!(second["items"][0]["traceId"], trace_id(1).to_string());
    assert!(second["nextCursor"].is_null());
    assert_eq!(
        get(&app, "/api/v1/traces?status=ok").await.1["items"][0]["traceId"],
        trace_id(3).to_string()
    );
    assert_eq!(
        get(&app, "/api/v1/traces?status=unset").await.1["items"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let (_, services) = get(&app, "/api/v1/services?limit=2").await;
    assert_eq!(services["items"][0]["name"], "alpha");
    assert_eq!(services["items"][1]["name"], "beta");
    let (_, last) = get(
        &app,
        &format!(
            "/api/v1/services?cursor={}",
            services["nextCursor"].as_str().unwrap()
        ),
    )
    .await;
    assert_eq!(
        last,
        json!({"items":[{"name":"gamma","spanCount":1,"traceCount":1}],"nextCursor":null})
    );
}

#[tokio::test]
async fn invalid_filters_missing_records_and_authority_are_explicit() {
    let (_dir, repo, app) = setup().await;
    add_single(&repo, 5, 10, None).await;
    let invalid_queries = [
        "limit=101",
        "limit=0",
        "cursor=???",
        "error=yes",
        "status=unknown",
        "sinceNs=-1",
        "untilNs=9223372036854775808",
        "sinceNs=10&untilNs=9",
        "durationMinNs=10&durationMaxNs=9",
        "traceId=00000000000000000000000000000000",
        "unknown=1",
        "limit=1&limit=2",
    ];
    for query in invalid_queries {
        error(
            &app,
            &format!("/api/v1/traces?{query}"),
            StatusCode::BAD_REQUEST,
            "INVALID_QUERY",
        )
        .await;
    }
    error(
        &app,
        "/api/v1/traces/invalid",
        StatusCode::BAD_REQUEST,
        "INVALID_TRACE_ID",
    )
    .await;
    error(
        &app,
        &format!("/api/v1/traces/{ID}"),
        StatusCode::NOT_FOUND,
        "TRACE_NOT_FOUND",
    )
    .await;
    error(
        &app,
        &format!("/api/v1/traces/{ID}/spans"),
        StatusCode::NOT_FOUND,
        "TRACE_NOT_FOUND",
    )
    .await;
    assert_eq!(
        get(&app, "/api/v1/traces?service=%27%20OR%201%3D1--")
            .await
            .1,
        json!({"items":[],"nextCursor":null})
    );
    assert_eq!(
        get(&app, "/api/v1/traces?route=%27%20OR%201%3D1--").await.1,
        json!({"items":[],"nextCursor":null})
    );
    assert!(bind("0.0.0.0:0".parse().unwrap()).await.is_err());
    assert!(bind("[::]:0".parse().unwrap()).await.is_err());
    for (name, value) in [
        ("host", "rebinding.example:4731"),
        ("host", "127.0.0.1:8000"),
        ("host", "user@127.0.0.1:4731"),
        ("origin", "http://rebinding.example:4731"),
        ("origin", "null"),
        ("origin", "http://127.0.0.1:4731/"),
        ("sec-fetch-site", "cross-site"),
    ] {
        let mut req = request("/api/v1/health");
        req.headers_mut().insert(name, value.parse().unwrap());
        assert_eq!(send(&app, req).await.0, StatusCode::FORBIDDEN);
    }
    let mut req = request("/api/v1/health");
    req.headers_mut()
        .insert(header::ORIGIN, format!("http://{HOST}").parse().unwrap());
    assert_eq!(send(&app, req).await.0, StatusCode::OK);
    let mut req = request("/api/v1/traces");
    *req.method_mut() = axum::http::Method::POST;
    assert_eq!(send(&app, req).await.0, StatusCode::METHOD_NOT_ALLOWED);

    repo.close().await;
    error(
        &app,
        "/api/v1/health",
        StatusCode::SERVICE_UNAVAILABLE,
        "STORAGE_UNAVAILABLE",
    )
    .await;
}

#[tokio::test]
async fn byte_budget_pages_large_spans_without_dropping_items() {
    let (_dir, repo, app) = setup().await;
    let mut groups = decode_export(SINGLE).unwrap();
    let span = &mut groups[0].scope_spans[0].spans[0];
    span.attributes.push(Attribute {
        key: "large".into(),
        value: Some(AttributeValue::String("é".repeat(3 * 1024 * 1024))),
    });
    let mut second = span.clone();
    second.span_id = xray_core::SpanId::try_from([2; 8]).unwrap();
    groups[0].scope_spans[0].spans.push(second);
    // Each row has just under 6 MiB of attribute text. Include a third row to
    // make the configured storage byte budget split the requested page.
    let mut third = groups[0].scope_spans[0].spans[1].clone();
    third.span_id = xray_core::SpanId::try_from([3; 8]).unwrap();
    groups[0].scope_spans[0].spans.push(third);
    repo.write_batch(groups).await.unwrap();
    let path = format!("/api/v1/traces/{ID}/spans");
    let (status, one) = get(&app, &path).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(one["items"].as_array().unwrap().len(), 1);
    let (_, two) = get(
        &app,
        &format!("{path}?cursor={}", one["nextCursor"].as_str().unwrap()),
    )
    .await;
    assert_eq!(two["items"].as_array().unwrap().len(), 1);
    let (_, last) = get(
        &app,
        &format!("{path}?cursor={}", two["nextCursor"].as_str().unwrap()),
    )
    .await;
    assert_eq!(last["items"].as_array().unwrap().len(), 1);
    assert!(last["nextCursor"].is_null());

    let mut oversized = decode_export(SINGLE).unwrap();
    let span = &mut oversized[0].scope_spans[0].spans[0];
    span.trace_id = trace_id(9);
    span.attributes.push(Attribute {
        key: "oversized".into(),
        value: Some(AttributeValue::String("x".repeat(12 * 1024 * 1024))),
    });
    repo.write_batch(oversized).await.unwrap();
    error(
        &app,
        &format!("/api/v1/traces/{}/spans?limit=1", trace_id(9)),
        StatusCode::PAYLOAD_TOO_LARGE,
        "RESPONSE_TOO_LARGE",
    )
    .await;
}
