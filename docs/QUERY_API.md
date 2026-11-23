# Local query API

Run `cargo run --locked -p xray-collector` to start OTLP ingestion and the read-only API. The query listener defaults to `http://127.0.0.1:4731`. Set `REQUEST_XRAY_QUERY_LISTEN` to an IPv4 or IPv6 loopback socket address to change it. Port 0 requests an OS-assigned port. Validation-only collector mode does not start a query listener.

```sh
curl http://127.0.0.1:4731/api/v1/health
curl 'http://127.0.0.1:4731/api/v1/traces?limit=20&error=true'
```

| Route | Result |
| --- | --- |
| `GET /api/v1/health` | Service version and SQLite readiness. |
| `GET /api/v1/traces` | Filtered trace summaries and an opaque next cursor. |
| `GET /api/v1/traces/{id}` | One trace summary, including timing, service count and completeness observations. |
| `GET /api/v1/traces/{id}/spans` | A page of persisted spans with events, links, attributes and resource and scope metadata. |
| `GET /api/v1/services` | A page of observed service names with span and distinct trace counts. |
| `GET /api/v1/stats` | Persisted trace, span, error-trace and service totals. The collector also exposes aggregate ingestion counters and the last retention pass. |
| `GET /api/v1/config/capabilities` | Feature availability and the maximum page size. |

## Filters and pagination

Trace filters accept exact `traceId`, `service`, and observed `route` (`http.route`) values. `error` is a boolean. `status` supports `error`, `ok`, and `unset`. An error on any persisted span takes precedence over an OK status. A trace with neither observation is unset. Filters combine with AND.

Use `sinceNs` and `untilNs` for inclusive lower and exclusive upper bounds on trace start time. `durationMinNs` and `durationMaxNs` are inclusive. Provide all bounds as nonnegative decimal nanosecond strings within the signed 64-bit range. Service and route filters accept up to 512 UTF-8 bytes each.

List responses contain `items` and `nextCursor`. Use the next cursor with the same filters until it is null. The default is 50 traces or services and 20 spans. The maximum is 100 records. Traces sort by descending start time and trace ID, spans by ascending start time and span ID, and services by name. A newer insertion does not shift an existing keyset page. Late spans can change a trace's start time between requests. Each page reflects committed data as of that request.

Span pages may return fewer records than requested with a non-null cursor to stay within a 12 MiB stored-payload budget. Span and service JSON responses have a 16 MiB limit. An oversized response returns HTTP 413 with `RESPONSE_TOO_LARGE`. Reduce the page limit if multiple records caused the overflow. A single oversized item produces the same explicit error.

## Precision, missing data, and errors

Timestamps and durations are decimal strings. Parse them with JavaScript `BigInt` when exact arithmetic matters. Attribute integers are also decimal strings, doubles are strings that can represent nonfinite values, and byte attributes use base64. Nullable fields such as parent span ID, route, status, and resource or scope metadata are explicit in the OpenAPI contract.

Completeness contains `state`, `missingParentCount`, and `lastSeenAtUnixNs`. State is `missing_parents` when a referenced parent is absent and `observed` otherwise. Observed data can still be incomplete. The API has no measurement of sampling coverage and reports that capability as unavailable.

Errors use `{"error":{"code":"TRACE_NOT_FOUND","message":"Trace not found.","requestId":"q-1"}}`. A malformed ID, filter or cursor returns HTTP 400, a missing trace returns 404, a forbidden request authority or origin returns 403, and a temporarily unavailable or saturated query service returns 503. The listener validates Host and any supplied Origin against the local listener, rejects cross-site fetch metadata, and sends no CORS permission headers. Responses include no-store, anti-sniffing and framing controls. The service allows eight concurrent requests and applies a five-second query deadline.

## Contract and client

The [OpenAPI 3.1 schema](../schemas/collector.openapi.yaml) is the source of truth. It uses JSON syntax, which is valid YAML 1.2. The private workspace package `@request-xray/query-client` generates operation and JSON types from it and exposes a typed Fetch client.

```sh
corepack pnpm --filter @request-xray/query-client generate
corepack pnpm --filter @request-xray/query-client build
corepack pnpm --filter @request-xray/query-client test
cargo test --offline --locked -p xray-api -p xray-storage -p xray-collector
```

Node CI checks the generated snapshot and TypeScript types. Rust CI exercises every route and its required top-level schema fields, filtering, precision, pagination across an intervening insertion, byte caps, migration backfill, and Host and Origin validation. The collector integration test confirms that the HTTP API reads committed data across process restart, reflects late parents, and exposes redacted spans.

On 2026-11-23, all 39 Rust tests, Clippy, formatting, TypeScript typechecking, the generated snapshot check, and the client build passed. The live OpenTelemetry exporter sent four spans and the generated client read all four through one-record pages, preserving one-nanosecond precision with JavaScript `BigInt`.
