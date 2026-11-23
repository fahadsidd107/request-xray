# OTLP/HTTP collector

The `xray-collector` crate exposes `POST /v1/traces` on `127.0.0.1:4318` by default. The native binary accepts loopback IPv4 or IPv6 addresses. The binary also starts the [query API](QUERY_API.md) on a separate loopback listener with SQLite enabled.

## Run and verify

Install the test lab dependencies with `corepack pnpm install --frozen-lockfile`. Start the collector locally:

```sh
cargo run --locked -p xray-collector
```

In another terminal, send deterministic spans from the official OpenTelemetry JavaScript protobuf exporter:

```sh
node test/scripts/generate-otlp-fixtures.cjs --verify-collector http://127.0.0.1:4318/v1/traces
```

Expect `Collector accepted single.pb` and `Collector accepted distributed.pb`. By default the collector writes to SQLite and acknowledges accepted spans after commit. Pass `--validate-only` to exercise ingestion and redaction and discard the batch. [SQLite configuration](STORAGE.md) [Acknowledgment ADR](adr/0008-acknowledge-ingestion-after-commit.md)

## Request and response contract

The endpoint accepts `Content-Type: application/x-protobuf` and either identity or one gzip content encoding. It returns an uncompressed protobuf response. Success uses `ExportTraceServiceResponse`. Errors use the message field of `google.rpc.Status`. [OTLP/HTTP response specification](https://opentelemetry.io/docs/specs/otlp/#otlphttp-response)

| HTTP status | Condition |
| --- | --- |
| 200 | The request contains no spans or the sink accepted every valid span. If some spans were invalid or conflicted, `partial_success.rejected_spans` reports their count. |
| 400 | The gzip or protobuf body is invalid, the request body cannot be read, or every span is invalid or conflicts with a committed span identity. |
| 408 | The admitted request exceeded the body read timeout. |
| 413 | A compressed, decompressed, protobuf field, nesting or trace structure limit was exceeded. The entire request is rejected. |
| 415 | The content type or content encoding is missing where required, ambiguous or unsupported. |
| 503 | Ingestion is full or shutting down, or the sink failed or timed out. Includes `Retry-After: 1`. |

## Configuration and limits

| Binary environment variable | Default | Effect |
| --- | --- | --- |
| `REQUEST_XRAY_OTLP_LISTEN` | `127.0.0.1:4318` | Loopback socket address. Port 0 lets the OS choose a free port. |
| `REQUEST_XRAY_MAX_COMPRESSED_BYTES` | 4 MiB | Maximum gzip request body size. |
| `REQUEST_XRAY_MAX_DECODED_BYTES` | 16 MiB | Maximum uncompressed protobuf size, including identity requests. |
| `REQUEST_XRAY_MAX_IN_FLIGHT` | 8 | Concurrent admitted requests. A value of zero rejects every ingest request. |
| `REQUEST_XRAY_BODY_TIMEOUT_MS` | 10000 | Time allowed to read an admitted request body. |
| `REQUEST_XRAY_SINK_TIMEOUT_MS` | 10000 | Time allowed for the sink to report its outcome. |
| `REQUEST_XRAY_ALLOW_ATTRIBUTES` | Unset | Optional comma-separated exact key allowlist. An empty value allows zero keys. |
| `REQUEST_XRAY_DENY_ATTRIBUTES` | Unset | Additional comma-separated exact keys to deny. |

The library's `Config::limits` also configures defaults of 100,000 protobuf fields, 10,000 spans per request, 128 events and 128 links per span, 128 attributes per list, 128 elements per attribute array, 16 KiB per attribute key or string/byte value, and eight levels of nested attribute containers. A wire scan counts fields before Prost allocates the decoded hierarchy and caps embedded message nesting at 64 levels. Structural limits apply before domain conversion and redaction. Size limits apply independently to each request.

## Redaction

The collector filters resource, scope, span, event and link attributes, including nested attribute lists. Default patterns cover credentials, authorization, cookies, headers and bodies, payment card keys, raw database query text and bind values, and exception messages and stack traces. Key matching is case-insensitive, and the default patterns recognize common separators. Default and configured denials take precedence over the optional allowlist.

The collector strips query strings, fragments and URL userinfo from recognized URL values and URL-like span or event names. It clears free-form status messages and preserves status codes and exporter-reported dropped counts. Redaction is key- and URL-based, so sensitive values under arbitrary keys or embedded in free-form text can survive. Collector logs and protocol error messages exclude request bodies and attribute values.

## Admission, shutdown and diagnostics

A permit covers body reading, decoding, redaction and the sink call. Admission never waits for a permit. An overloaded request is rejected before body buffering. The native server stops accepting connections on Ctrl-C or SIGTERM on Unix and drains already admitted requests under the same body and sink timeouts. A write timeout drops the sink future. A transaction may already have committed by that point, so the SQLite sink must support idempotent retries.

`Ingestion::diagnostics()` returns aggregate counters for the fixed source endpoint `/v1/traces`. The binary prints the snapshot on graceful shutdown. It contains no client addresses, telemetry IDs, attribute names or values.

- Accepted spans count successful sink outcomes, including duplicates. Duplicate spans count the subset reported by the sink. Conflicts count separately and are also included in rejected spans. Validation-only spans have their own counter.
- Rejected spans count individual domain validation failures. Requests rejected before domain conversion are accounted for separately as size, invalid payload, admission, shutdown or body timeout events.
- Redacted spans count spans with changes to their own fields, events or links. Redacted fields also include resource and scope changes. Redaction counts reflect work done even if the sink subsequently fails.
- Write failures include sink errors and timeouts. Write timeouts are also counted separately.

## Verification results

On 2026-11-20, the live exporter sent both test batches to a collector bound to an OS-assigned loopback port in validation-only mode. SIGTERM exited cleanly and printed four validation-only spans, one redacted span and zero committed spans. Run `cargo test --offline --locked -p xray-collector` for ten passing integration tests covering loopback binding, protobuf full and partial success, malformed input, gzip and structural limits, redaction, saturation, acknowledgment ordering, timeouts, counters and live connection drain.

The [storage verification](STORAGE.md#verification) covers acknowledgments with SQLite enabled and process restart.
