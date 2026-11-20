//! Loopback OTLP/HTTP ingestion and validation.

mod admission;
mod limits;
mod preflight;
mod redaction;

pub use admission::{Admission, Diagnostics, Ingestion, SinkOutcome};
pub use limits::Limits;
pub use redaction::Redaction;

use std::{error::Error, future::Future, io, io::Read, net::SocketAddr, sync::Arc};

use axum::{
    body::to_bytes,
    extract::{Request, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Router,
};
use flate2::read::MultiGzDecoder;
use prost::Message;
use xray_core::ResourceSpans;
use xray_otlp::{
    convert_export_report,
    opentelemetry::proto::collector::trace::v1::{
        ExportTracePartialSuccess, ExportTraceServiceRequest, ExportTraceServiceResponse,
    },
};

const PROTOBUF: &str = "application/x-protobuf";

#[derive(Clone)]
pub struct Config {
    pub listen: SocketAddr,
    pub limits: Limits,
    pub redaction: Redaction,
    pub admission: Admission,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            listen: SocketAddr::from(([127, 0, 0, 1], 4318)),
            limits: Limits::default(),
            redaction: Redaction::default(),
            admission: Admission::default(),
        }
    }
}

#[derive(Debug)]
pub struct StorageUnavailable;

impl Config {
    /// Bind only to an IPv4 or IPv6 loopback address.
    pub async fn bind(&self) -> io::Result<tokio::net::TcpListener> {
        if !self.listen.ip().is_loopback() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "OTLP listener address must be loopback",
            ));
        }
        tokio::net::TcpListener::bind(self.listen).await
    }
}

/// Build the receiver. The sink receives only sanitized canonical spans.
/// Returning success from the sink permits the HTTP handler to acknowledge them.
pub fn router<S, F>(config: Config, sink: S) -> Router
where
    S: Fn(Vec<ResourceSpans>) -> F + Send + Sync + 'static,
    F: Future<Output = Result<(), StorageUnavailable>> + Send + 'static,
{
    Ingestion::new(config).router(move |batch| {
        let result = sink(batch);
        async move {
            result.await?;
            Ok(SinkOutcome::Committed { duplicate_spans: 0 })
        }
    })
}

impl Ingestion {
    /// Build a router with a sink that reports its commit outcome. The sink must
    /// handle cancellation and retrying a batch after a lost acknowledgment.
    pub fn router<S, F>(&self, sink: S) -> Router
    where
        S: Fn(Vec<ResourceSpans>) -> F + Send + Sync + 'static,
        F: Future<Output = Result<SinkOutcome, StorageUnavailable>> + Send + 'static,
    {
        Router::new()
            .route(Diagnostics::SOURCE_ENDPOINT, post(ingest::<S, F>))
            .method_not_allowed_fallback(|| async {
                let mut response = failure(StatusCode::METHOD_NOT_ALLOWED, "method_not_allowed");
                response
                    .headers_mut()
                    .insert(header::ALLOW, "POST".parse().expect("valid header"));
                response
            })
            .fallback(|| async { failure(StatusCode::NOT_FOUND, "not_found") })
            .with_state((self.clone(), Arc::new(sink)))
    }

    /// Serve a configured listener until shutdown, then drain admitted requests.
    pub async fn serve<S, F>(
        &self,
        listener: tokio::net::TcpListener,
        sink: S,
        shutdown: impl Future<Output = ()> + Send + 'static,
    ) -> io::Result<()>
    where
        S: Fn(Vec<ResourceSpans>) -> F + Send + Sync + 'static,
        F: Future<Output = Result<SinkOutcome, StorageUnavailable>> + Send + 'static,
    {
        if !listener.local_addr()?.ip().is_loopback() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "OTLP listener address must be loopback",
            ));
        }
        let ingestion = self.clone();
        axum::serve(listener, self.router(sink))
            .with_graceful_shutdown(async move {
                shutdown.await;
                ingestion.begin_shutdown();
            })
            .await
    }
}

async fn ingest<S, F>(
    State((ingestion, sink)): State<(Ingestion, Arc<S>)>,
    request: Request,
) -> Response
where
    S: Fn(Vec<ResourceSpans>) -> F + Send + Sync + 'static,
    F: Future<Output = Result<SinkOutcome, StorageUnavailable>> + Send + 'static,
{
    let (parts, body) = request.into_parts();
    let headers = &parts.headers;

    if !matches!(single_header(headers, header::CONTENT_TYPE), Ok(Some(media)) if media.eq_ignore_ascii_case(PROTOBUF))
    {
        return failure(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "unsupported_content_type",
        );
    }
    let gzip = match single_header(headers, header::CONTENT_ENCODING) {
        Ok(None) => false,
        Ok(Some(encoding)) if encoding.eq_ignore_ascii_case("identity") => false,
        Ok(Some(encoding)) if encoding.eq_ignore_ascii_case("gzip") => true,
        _ => {
            return failure(
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "unsupported_content_encoding",
            )
        }
    };

    let _permit = match ingestion.slots.try_acquire() {
        Ok(permit) => permit,
        Err(tokio::sync::TryAcquireError::NoPermits) => {
            ingestion.record(|stats| stats.admission_rejections += 1);
            return retryable_failure("resource_limit: ingestion capacity exhausted");
        }
        Err(tokio::sync::TryAcquireError::Closed) => {
            ingestion.record(|stats| stats.shutdown_rejections += 1);
            return retryable_failure("collector_shutting_down");
        }
    };
    let config = &ingestion.config;
    let body_limit = if gzip {
        config.limits.compressed_bytes
    } else {
        config.limits.decoded_bytes
    };
    let bytes =
        match tokio::time::timeout(config.admission.body_timeout, to_bytes(body, body_limit)).await
        {
            Ok(Ok(bytes)) => bytes,
            Ok(Err(error))
                if error
                    .source()
                    .is_some_and(|source| source.is::<http_body_util::LengthLimitError>()) =>
            {
                return ingestion.size_error("resource_limit");
            }
            Ok(Err(_)) => return ingestion.invalid_payload("invalid_payload"),
            Err(_) => {
                ingestion.record(|stats| stats.body_timeouts += 1);
                return failure(StatusCode::REQUEST_TIMEOUT, "request_timeout");
            }
        };
    let mut decompressed = Vec::new();
    let payload = if gzip {
        let result = MultiGzDecoder::new(bytes.as_ref())
            .take(config.limits.decoded_bytes.saturating_add(1) as u64)
            .read_to_end(&mut decompressed);
        if decompressed.len() > config.limits.decoded_bytes {
            return ingestion.size_error("resource_limit");
        }
        if result.is_err() {
            return ingestion.invalid_payload("invalid_payload: invalid gzip body");
        }
        decompressed.as_slice()
    } else {
        bytes.as_ref()
    };

    match preflight::check(payload, config.limits.protobuf_fields) {
        Ok(()) => {}
        Err(preflight::PreflightError::Invalid) => {
            return ingestion.invalid_payload("invalid_payload: invalid trace protobuf");
        }
        Err(preflight::PreflightError::Limit) => {
            return ingestion
                .size_error("resource_limit: protobuf field or nesting limit exceeded");
        }
    }
    let Ok(request) = ExportTraceServiceRequest::decode(payload) else {
        return ingestion.invalid_payload("invalid_payload: invalid trace protobuf");
    };
    if !config.limits.permits(&request) {
        return ingestion.size_error("resource_limit: trace structure exceeds configured limits");
    }

    drop(bytes);
    drop(decompressed);
    let mut report = convert_export_report(request);
    let valid_spans: usize = report
        .resource_spans
        .iter()
        .flat_map(|resource| &resource.scope_spans)
        .map(|scope| scope.spans.len())
        .sum();
    ingestion.record(|stats| stats.rejected_spans += report.rejected_spans as u64);

    if valid_spans != 0 {
        let (redacted_spans, redacted_fields) =
            config.redaction.sanitize(&mut report.resource_spans);
        ingestion.record(|stats| {
            stats.redacted_spans += redacted_spans;
            stats.redacted_fields += redacted_fields;
        });
        match tokio::time::timeout(config.admission.sink_timeout, async {
            sink(report.resource_spans).await
        })
        .await
        {
            Ok(Ok(SinkOutcome::Committed { duplicate_spans })) => {
                ingestion.record(|stats| {
                    stats.accepted_spans += valid_spans as u64;
                    stats.duplicate_spans += duplicate_spans.min(valid_spans) as u64;
                });
            }
            Ok(Ok(SinkOutcome::ValidationOnly)) => {
                ingestion.record(|stats| stats.validation_only_spans += valid_spans as u64);
            }
            Ok(Err(_)) => {
                ingestion.record(|stats| stats.write_failures += 1);
                return retryable_failure("storage_unavailable");
            }
            Err(_) => {
                ingestion.record(|stats| {
                    stats.write_failures += 1;
                    stats.write_timeouts += 1;
                });
                return retryable_failure("storage_unavailable: write timeout");
            }
        }
    } else if report.rejected_spans != 0 {
        return failure(
            StatusCode::BAD_REQUEST,
            "invalid_span: all spans failed validation",
        );
    }

    protobuf_response(
        StatusCode::OK,
        ExportTraceServiceResponse {
            partial_success: (report.rejected_spans != 0).then(|| ExportTracePartialSuccess {
                // The protobuf input byte count bounds the number of spans.
                rejected_spans: report.rejected_spans as i64,
                error_message: "invalid_span: invalid ID or timestamp".into(),
            }),
        },
    )
}

impl Ingestion {
    fn size_error(&self, message: &str) -> Response {
        self.record(|stats| stats.size_errors += 1);
        failure(StatusCode::PAYLOAD_TOO_LARGE, message)
    }

    fn invalid_payload(&self, message: &str) -> Response {
        self.record(|stats| stats.invalid_payloads += 1);
        failure(StatusCode::BAD_REQUEST, message)
    }
}

fn retryable_failure(message: &str) -> Response {
    let mut response = failure(StatusCode::SERVICE_UNAVAILABLE, message);
    response
        .headers_mut()
        .insert(header::RETRY_AFTER, header::HeaderValue::from_static("1"));
    response
}

fn single_header(headers: &HeaderMap, name: header::HeaderName) -> Result<Option<&str>, ()> {
    let values = headers.get_all(name);
    let mut values = values.iter();
    let first = values.next();
    if values.next().is_some() {
        return Err(());
    }
    first
        .map(|value| value.to_str().map(str::trim).map_err(|_| ()))
        .transpose()
}

// google.rpc.Status wire field 2. Code and details are optional for OTLP.
#[derive(Clone, PartialEq, Message)]
struct RpcStatus {
    #[prost(string, tag = "2")]
    message: String,
}

fn failure(status: StatusCode, message: &str) -> Response {
    protobuf_response(
        status,
        RpcStatus {
            message: message.into(),
        },
    )
}

fn protobuf_response(status: StatusCode, message: impl Message) -> Response {
    (
        status,
        [(header::CONTENT_TYPE, PROTOBUF)],
        message.encode_to_vec(),
    )
        .into_response()
}
