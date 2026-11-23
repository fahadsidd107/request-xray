//! Read-only loopback HTTP API for persisted traces.

mod model;

pub use model::{IngestionStats, RetentionStats, RuntimeStats};

use std::{
    future::Future,
    io,
    net::SocketAddr,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use axum::{
    extract::{
        rejection::{PathRejection, QueryRejection},
        FromRef, Path, Query, Request, State,
    },
    http::{header, uri::Authority, HeaderMap, HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use model::{Page, SpanItem, Stats, Trace};
use serde::{Deserialize, Serialize};
use tokio::{net::TcpListener, sync::Semaphore};
use xray_core::{SpanId, TraceId};
use xray_storage::{Repository, StorageError, TraceCursor, TraceFilter, TraceStatus};

const MAX_PAGE: u32 = 100;
const MAX_QUERY_BYTES: usize = 32 * 1024;
const MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;
const MAX_STORED_PAGE_BYTES: i64 = 12 * 1024 * 1024;
static REQUEST_NUMBER: AtomicU64 = AtomicU64::new(1);

pub const DEFAULT_LISTEN: SocketAddr =
    SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), 4731);

pub async fn bind(address: SocketAddr) -> io::Result<TcpListener> {
    validate_address(address)?;
    TcpListener::bind(address).await
}

fn validate_address(address: SocketAddr) -> io::Result<()> {
    if address.ip().is_loopback() {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "query listener must be loopback",
        ))
    }
}

pub type DiagnosticsSource = Arc<dyn Fn() -> RuntimeStats + Send + Sync>;

#[derive(Clone)]
struct ApiState {
    repository: Repository,
    diagnostics: Option<DiagnosticsSource>,
}

impl FromRef<ApiState> for Repository {
    fn from_ref(state: &ApiState) -> Self {
        state.repository.clone()
    }
}

pub async fn serve(
    listener: TcpListener,
    repository: Repository,
    diagnostics: Option<DiagnosticsSource>,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> io::Result<()> {
    let app = router_with_diagnostics(repository, listener.local_addr()?, diagnostics)?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await
}

#[derive(Clone)]
struct Gate {
    address: SocketAddr,
    slots: Arc<Semaphore>,
}

pub fn router(repository: Repository, address: SocketAddr) -> io::Result<Router> {
    router_with_diagnostics(repository, address, None)
}

fn router_with_diagnostics(
    repository: Repository,
    address: SocketAddr,
    diagnostics: Option<DiagnosticsSource>,
) -> io::Result<Router> {
    validate_address(address)?;
    let gate = Gate {
        address,
        slots: Arc::new(Semaphore::new(8)),
    };
    Ok(Router::new()
        .route("/api/v1/health", get(health))
        .route("/api/v1/traces", get(traces))
        .route("/api/v1/traces/{id}", get(trace))
        .route("/api/v1/traces/{id}/spans", get(spans))
        .route("/api/v1/services", get(services))
        .route("/api/v1/stats", get(stats))
        .route("/api/v1/config/capabilities", get(capabilities))
        .method_not_allowed_fallback(|| async {
            let mut response = ApiError(
                StatusCode::METHOD_NOT_ALLOWED,
                "METHOD_NOT_ALLOWED",
                "Use GET or HEAD.",
            )
            .into_response();
            response
                .headers_mut()
                .insert(header::ALLOW, HeaderValue::from_static("GET, HEAD"));
            response
        })
        .fallback(|| async { ApiError(StatusCode::NOT_FOUND, "NOT_FOUND", "Route not found.") })
        .with_state(ApiState {
            repository,
            diagnostics,
        })
        .layer(middleware::from_fn_with_state(gate, guard)))
}

async fn guard(State(gate): State<Gate>, request: Request, next: Next) -> Response {
    let headers = request.headers();
    let mut response =
        if request.uri().authority().is_some() || !allowed_authority(headers, gate.address) {
            ApiError(
                StatusCode::FORBIDDEN,
                "FORBIDDEN",
                "Use the local query listener origin.",
            )
            .into_response()
        } else if request
            .uri()
            .query()
            .is_some_and(|query| query.len() > MAX_QUERY_BYTES)
        {
            invalid().into_response()
        } else if let Ok(_permit) = gate.slots.try_acquire() {
            match tokio::time::timeout(Duration::from_secs(5), next.run(request)).await {
                Ok(response) => response,
                Err(_) => unavailable().into_response(),
            }
        } else {
            unavailable().into_response()
        };
    for (name, value) in [
        ("cache-control", "no-store"),
        ("x-content-type-options", "nosniff"),
        ("x-frame-options", "DENY"),
        ("referrer-policy", "no-referrer"),
        ("cross-origin-resource-policy", "same-origin"),
        (
            "content-security-policy",
            "default-src 'none'; frame-ancestors 'none'",
        ),
    ] {
        response
            .headers_mut()
            .insert(name, HeaderValue::from_static(value));
    }
    response
}

fn one_header<'a>(headers: &'a HeaderMap, name: &str) -> Option<Option<&'a str>> {
    let mut entries = headers.get_all(name).iter();
    let Some(first) = entries.next() else {
        return Some(None);
    };
    if entries.next().is_some() {
        return None;
    }
    first.to_str().ok().map(Some)
}

fn allowed_authority(headers: &HeaderMap, address: SocketAddr) -> bool {
    let Some(Some(host)) = one_header(headers, "host") else {
        return false;
    };
    let Ok(authority) = host.parse::<Authority>() else {
        return false;
    };
    if authority.as_str().contains('@')
        || (authority.port().is_some() && authority.port_u16().is_none())
        || authority.port_u16().unwrap_or(80) != address.port()
    {
        return false;
    }
    let hostname = authority.host();
    if !hostname.eq_ignore_ascii_case("localhost")
        && hostname
            .trim_matches(['[', ']'])
            .parse::<std::net::IpAddr>()
            .ok()
            != Some(address.ip())
    {
        return false;
    }
    if !matches!(
        one_header(headers, "sec-fetch-site"),
        Some(None | Some("same-origin" | "none"))
    ) {
        return false;
    }
    match one_header(headers, "origin") {
        Some(None) => true,
        Some(Some(origin)) => origin
            .strip_prefix("http://")
            .is_some_and(|origin| origin.eq_ignore_ascii_case(host)),
        None => false,
    }
}

struct ApiError(StatusCode, &'static str, &'static str);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let request_id = format!("q-{}", REQUEST_NUMBER.fetch_add(1, Ordering::Relaxed));
        (
            self.0,
            Json(serde_json::json!({
                "error": { "code": self.1, "message": self.2, "requestId": request_id }
            })),
        )
            .into_response()
    }
}

fn invalid() -> ApiError {
    ApiError(
        StatusCode::BAD_REQUEST,
        "INVALID_QUERY",
        "Invalid query parameters or cursor.",
    )
}
fn unavailable() -> ApiError {
    ApiError(
        StatusCode::SERVICE_UNAVAILABLE,
        "STORAGE_UNAVAILABLE",
        "Query service is temporarily unavailable.",
    )
}
fn missing() -> ApiError {
    ApiError(StatusCode::NOT_FOUND, "TRACE_NOT_FOUND", "Trace not found.")
}
fn too_large() -> ApiError {
    ApiError(StatusCode::PAYLOAD_TOO_LARGE, "RESPONSE_TOO_LARGE", "This page exceeds the response size limit. Try a smaller limit. A single oversized item cannot be returned.")
}
fn storage_error(error: StorageError) -> ApiError {
    match error {
        StorageError::PageTooLarge => too_large(),
        StorageError::InvalidPageSize => invalid(),
        _ => unavailable(),
    }
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TraceQuery {
    limit: Option<u32>,
    cursor: Option<String>,
    trace_id: Option<String>,
    service: Option<String>,
    route: Option<String>,
    error: Option<bool>,
    status: Option<StatusFilter>,
    since_ns: Option<String>,
    until_ns: Option<String>,
    duration_min_ns: Option<String>,
    duration_max_ns: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum StatusFilter {
    Unset,
    Ok,
    Error,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PageQuery {
    limit: Option<u32>,
    cursor: Option<String>,
}

fn limit(value: Option<u32>, default: u32) -> Result<u32, ApiError> {
    let value = value.unwrap_or(default);
    (1..=MAX_PAGE)
        .contains(&value)
        .then_some(value)
        .ok_or_else(invalid)
}

fn nanos(value: &str) -> Result<i64, ApiError> {
    if value.is_empty() || value.len() > 19 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(invalid());
    }
    value.parse().map_err(|_| invalid())
}

fn encode_cursor(kind: &str, fields: &str) -> String {
    URL_SAFE_NO_PAD.encode(format!("v1:{kind}:{fields}"))
}

fn decode_cursor(token: &str, prefix: &str) -> Result<String, ApiError> {
    if token.len() > MAX_QUERY_BYTES {
        return Err(invalid());
    }
    let bytes = URL_SAFE_NO_PAD.decode(token).map_err(|_| invalid())?;
    let fields = std::str::from_utf8(&bytes).map_err(|_| invalid())?;
    fields
        .strip_prefix(prefix)
        .filter(|tail| !tail.is_empty())
        .map(str::to_owned)
        .ok_or_else(invalid)
}

fn checked_text(text: Option<&str>) -> Result<Option<&str>, ApiError> {
    if text.is_some_and(|value| {
        value.is_empty() || value.len() > 512 || value.chars().any(char::is_control)
    }) {
        Err(invalid())
    } else {
        Ok(text)
    }
}

async fn traces(
    State(repo): State<Repository>,
    query: Result<Query<TraceQuery>, QueryRejection>,
) -> Result<Json<Page<Trace>>, ApiError> {
    let Query(query) = query.map_err(|_| invalid())?;
    let after = query
        .cursor
        .as_deref()
        .map(|token| {
            let fields = decode_cursor(token, "v1:t:")?;
            let (start, id) = fields.split_once(':').ok_or_else(invalid)?;
            Ok(TraceCursor {
                start_ns: nanos(start)?,
                trace_id: id.parse().map_err(|_| invalid())?,
            })
        })
        .transpose()?;
    let since_ns = query.since_ns.as_deref().map(nanos).transpose()?;
    let until_ns = query.until_ns.as_deref().map(nanos).transpose()?;
    let min_duration_ns = query.duration_min_ns.as_deref().map(nanos).transpose()?;
    let max_duration_ns = query.duration_max_ns.as_deref().map(nanos).transpose()?;
    if matches!((since_ns, until_ns), (Some(lower), Some(upper)) if lower >= upper)
        || matches!((min_duration_ns, max_duration_ns), (Some(lower), Some(upper)) if lower > upper)
    {
        return Err(invalid());
    }
    let page = repo
        .list_traces(
            TraceFilter {
                trace_id: query
                    .trace_id
                    .as_deref()
                    .map(str::parse)
                    .transpose()
                    .map_err(|_| invalid())?,
                route: checked_text(query.route.as_deref())?,
                status: query.status.map(|status| match status {
                    StatusFilter::Unset => TraceStatus::Unset,
                    StatusFilter::Ok => TraceStatus::Ok,
                    StatusFilter::Error => TraceStatus::Error,
                }),
                min_duration_ns,
                max_duration_ns,
                service: checked_text(query.service.as_deref())?,
                has_error: query.error,
                since_ns,
                until_ns,
                after,
            },
            limit(query.limit, 50)?,
        )
        .await
        .map_err(storage_error)?;
    Ok(Json(Page {
        items: page.traces.into_iter().map(Trace::from).collect(),
        next_cursor: page
            .next
            .map(|cursor| encode_cursor("t", &format!("{}:{}", cursor.start_ns, cursor.trace_id))),
    }))
}

fn trace_id(path: Result<Path<String>, PathRejection>) -> Result<TraceId, ApiError> {
    let Path(id) = path.map_err(|_| invalid())?;
    id.parse().map_err(|_| {
        ApiError(
            StatusCode::BAD_REQUEST,
            "INVALID_TRACE_ID",
            "Invalid trace identifier.",
        )
    })
}

async fn trace(
    State(repo): State<Repository>,
    path: Result<Path<String>, PathRejection>,
) -> Result<Json<Trace>, ApiError> {
    let record = repo
        .get_trace(trace_id(path)?)
        .await
        .map_err(storage_error)?
        .ok_or_else(missing)?;
    Ok(Json(record.into()))
}

async fn spans(
    State(repo): State<Repository>,
    path: Result<Path<String>, PathRejection>,
    query: Result<Query<PageQuery>, QueryRejection>,
) -> Result<Response, ApiError> {
    let trace = trace_id(path)?;
    let Query(query) = query.map_err(|_| invalid())?;
    let prefix = format!("v1:s:{trace}:");
    let after = query
        .cursor
        .as_deref()
        .map(|token| {
            let fields = decode_cursor(token, &prefix)?;
            let (start, id) = fields.split_once(':').ok_or_else(invalid)?;
            Ok((nanos(start)?, id.parse::<SpanId>().map_err(|_| invalid())?))
        })
        .transpose()?;
    let (items, more) = repo
        .span_page(trace, after, limit(query.limit, 20)?, MAX_STORED_PAGE_BYTES)
        .await
        .map_err(storage_error)?
        .ok_or_else(missing)?;
    let next_cursor = if more {
        items.last().map(|row| {
            encode_cursor(
                "s",
                &format!("{trace}:{}:{}", row.span.interval.start(), row.span.span_id),
            )
        })
    } else {
        None
    };
    bounded_json(Page {
        items: items.into_iter().map(SpanItem::from).collect(),
        next_cursor,
    })
}

fn bounded_json(data: impl Serialize) -> Result<Response, ApiError> {
    struct Limited(Vec<u8>);
    impl io::Write for Limited {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if bytes.len() > MAX_RESPONSE_BYTES - self.0.len() {
                return Err(io::Error::other("response byte cap"));
            }
            self.0.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut buffer = Limited(Vec::new());
    serde_json::to_writer(&mut buffer, &data).map_err(|_| too_large())?;
    Ok(([(header::CONTENT_TYPE, "application/json")], buffer.0).into_response())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Service {
    name: String,
    span_count: i64,
    trace_count: i64,
}

async fn services(
    State(repo): State<Repository>,
    query: Result<Query<PageQuery>, QueryRejection>,
) -> Result<Response, ApiError> {
    let Query(query) = query.map_err(|_| invalid())?;
    let after = query
        .cursor
        .as_deref()
        .map(|token| decode_cursor(token, "v1:n:"))
        .transpose()?;
    let (items, next) = repo
        .list_services(after.as_deref(), limit(query.limit, 50)?)
        .await
        .map_err(storage_error)?;
    bounded_json(Page {
        items: items
            .into_iter()
            .map(|row| Service {
                name: row.name,
                span_count: row.span_count,
                trace_count: row.trace_count,
            })
            .collect(),
        next_cursor: next.map(|id| encode_cursor("n", &id)),
    })
}

async fn health(State(repo): State<Repository>) -> Result<Json<serde_json::Value>, ApiError> {
    repo.ping().await.map_err(storage_error)?;
    Ok(Json(
        serde_json::json!({ "status": "ok", "version": env!("CARGO_PKG_VERSION") }),
    ))
}

async fn stats(State(state): State<ApiState>) -> Result<Json<Stats>, ApiError> {
    let mut stats = Stats::from(state.repository.stats().await.map_err(storage_error)?);
    stats.runtime = state.diagnostics.map(|source| source());
    Ok(Json(stats))
}

async fn capabilities(State(state): State<ApiState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "persistedTraces": true, "routeFilter": true, "spanEvents": true, "spanLinks": true,
        "traceCompleteness": true, "ingestionCounters": state.diagnostics.is_some(),
        "retentionCounters": state.diagnostics.is_some(), "samplingCoverage": false, "maxPageSize": MAX_PAGE
    }))
}
