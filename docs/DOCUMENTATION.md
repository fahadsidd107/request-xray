# Request X-Ray — Developer, User, CLI, SDK and Operations Documentation

> **Specification / future documentation.** Request X-Ray has **not yet been implemented or published**. Every `request-xray` command, `@request-xray/node` import, URL, JSON response and UI walkthrough is **proposed**, unless explicitly labeled as an existing third-party tool. The code in this document is a target contract for the implementation roadmap. Do not copy these as working commands until matching roadmap acceptance tests pass.

**Audience:** package users, operators, contributors, maintainers, and AI implementation agents. For architecture see [ARCHITECTURE.md](ARCHITECTURE.md); for work order and quality gates see [ROADMAP.md](ROADMAP.md).

## 1. Terminology

| Term | Meaning |
|---|---|
| Trace | Set of related spans sharing a trace ID; may be incomplete |
| Span | One instrumented operation with identity, timing, metadata, and parent/link context |
| Parent | A synchronous/nested causal relationship when recorded |
| Link | Association between spans, often useful for asynchronous work |
| Collector | Rust HTTP server receiving and validating OTLP telemetry |
| OTLP | OpenTelemetry Protocol, the data transport format |
| SDK | Instrumentation within the monitored application |
| Resource | Identity of telemetry producer (service name, version, environment) |
| Waterfall | Timeline of observed span time intervals and relationships |
| Evidence | Trace/span references plus measurements supporting an analysis rule |
| Gap | Missing or inconsistent context; not proof a service did not execute |
| Local mode | Both dashboard and collector listen on loopback, no authentication exposed to networks |

## 2. Who should use this?

Request X-Ray is designed for developers who need to investigate application requests locally and, eventually, in securely configured distributed environments. First-class initial audience: Node.js, Express, NestJS and Fastify developers. Non-Node systems can participate using their own OpenTelemetry SDK exporters pointed at Request X-Ray, once tested for protocol compatibility.

Do not use the pre-release collector with real customer/payment/medical telemetry. Do not expose a localhost-only unsecured collector/dashboard to external networks. Do not assume a trace contains all code executed in an application.

## 3. Prerequisites and intended support

- To **use** a future npm release: a supported Node.js LTS installation and npm or pnpm; supported OS/architecture with a precompiled Rust binary. No Rust compiler needed at runtime.
- To **contribute**: Rust stable toolchain, Node.js LTS, pnpm via Corepack, Git, and optional Docker for integration examples.
- Intended initial OS: macOS arm64/x64, Linux x64/arm64 glibc, Windows x64. Others unsupported until CI proof.
- Exact tested Node and Rust versions belong in the root manifests and compatibility matrix established in Step 0; don't invent version support now.

### Initial development environment (once scaffold exists)

```bash
# Existing ecosystem commands; root manifests/scripts do NOT exist yet
rustup show
cargo --version
node --version
corepack --version
corepack pnpm --version

# Future repository
pnpm install --frozen-lockfile
cargo test --workspace
pnpm -r typecheck
pnpm -r test
```

These will become a real bootstrap procedure after Step 0. Windows users may need PowerShell-equivalent paths; avoid Bash-only scripts in the npm CLI implementation.

## 4. Installation flows (planned)

### A. Existing Node application, easiest path

```bash
npm install @request-xray/node
npx request-xray init
npx request-xray dev
```

Expected intended behavior:

1. `init` inspects `package.json` and frameworks without modifying files silently. It proposes an instrumentation entrypoint and config, requests confirmation before rewriting scripts, and explains which libraries it can instrument.
2. `dev` starts local Rust collector on `127.0.0.1:4318` and dashboard/API on `127.0.0.1:4731`; exits nonzero with useful diagnostics on port conflicts.
3. Developer starts their application with the SDK **preloaded before imports**.
4. Requests emit OTLP protobuf to `/v1/traces`; they appear in the dashboard after exporter batching/flush.
5. `doctor` checks connectivity and likely startup problems.

`npm install @request-xray/node` alone will not magically instrument an already-running application; correct startup and exporter configuration are mandatory.

### B. Existing OpenTelemetry application

No new Request X-Ray SDK required. Configure the existing OTLP/HTTP **protobuf** trace exporter to target:

```text
http://127.0.0.1:4318/v1/traces
```

If the exporter supports a *base endpoint*, confirm its behavior (some append `/v1/traces`, while trace-specific endpoint options already expect the full path). Avoid accidental `/v1/traces/v1/traces`. Select `application/x-protobuf`; JSON OTLP and gRPC are not v0.1 requirements. Compatible exporter apps can be in Java, Go, Python, or Node.js when integration is verified.

### C. Run the demonstration

```bash
npx request-xray demo
```

Planned: launch synthetic ShopFlow-like services and make one reproducible request. All data is test data; dashboard clearly labels demo mode. No external payment/production APIs.

## 5. Configuration reference (proposed)

Proposed `request-xray.config.json` created by `init`:

```json
{
  "$schema": "./node_modules/request-xray/config.schema.json",
  "mode": "local",
  "collector": {
    "host": "127.0.0.1",
    "httpPort": 4318,
    "maxCompressedBodyBytes": 4194304,
    "maxDecompressedBodyBytes": 16777216
  },
  "dashboard": { "host": "127.0.0.1", "port": 4731 },
  "storage": {
    "driver": "sqlite",
    "path": ".request-xray/traces.sqlite",
    "retentionDays": 7,
    "maxSizeMiB": 500
  },
  "privacy": {
    "captureBodies": false,
    "captureRawHeaders": false,
    "captureSqlParameters": false,
    "dropUrlQuery": true,
    "denyAttributes": ["authorization", "cookie", "set-cookie", "password", "token"]
  },
  "analysis": { "rulesEnabled": true, "aiEnabled": false }
}
```

Values are **design defaults**, not validated performance recommendations. Final schema names/limits need ADRs, tests, and implementation. Default `captureBodies=false` is not a complete data-security solution; spans supplied by third-party instrumentation can still contain sensitive attributes, so collector redaction is also required.

### Configuration precedence

1. Explicit CLI flag (e.g. `--dashboard-port 4732`).
2. Environment variable.
3. Project configuration file.
4. Defined defaults.

The CLI must show which config file and profile were loaded without printing secrets.

### Planned environment variables

| Variable | Purpose | Default target |
|---|---|---|
| `REQUEST_XRAY_CONFIG` | Explicit config file path | `./request-xray.config.json` if present |
| `REQUEST_XRAY_DB_PATH` | Local SQLite file path | `.request-xray/traces.sqlite` |
| `REQUEST_XRAY_COLLECTOR_PORT` | Ingestion port | `4318` |
| `REQUEST_XRAY_DASHBOARD_PORT` | Dashboard/query port | `4731` |
| `REQUEST_XRAY_LOG_LEVEL` | Rust server log verbosity | `info` |
| `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT` | Standard OpenTelemetry SDK trace endpoint (SDK dependent) | `http://127.0.0.1:4318/v1/traces` |
| `OTEL_SERVICE_NAME` | Standard OTel service naming mechanism | app-specific |
| `OTEL_TRACES_SAMPLER` | Standard OTel sampler selector | SDK-defined |

Standard OTel variables are interpreted by the emitting SDK, not automatically by the Rust collector. Port overrides must be propagated consistently to SDK and server configuration.

## 6. Complete CLI command reference (proposed)

**These CLI commands are acceptance targets; help text, flags, and exit codes should be implemented and tested before being presented as functional.**

| Command | Purpose | Key intended flags | Expected outcome |
|---|---|---|---|
| `request-xray --help` | Discover commands | `--version` | Documentation, supported commands |
| `request-xray init` | Generate app wiring/config | `--framework`, `--yes`, `--dry-run` | Safe proposed files/diffs |
| `request-xray dev` | Start collector + API/dashboard | `--config`, `--collector-port`, `--dashboard-port`, `--no-open` | Local listeners and process logs |
| `request-xray doctor` | Diagnose telemetry setup | `--json`, `--config` | Specific pass/warn/fail checks |
| `request-xray demo` | Start synthetic example | `--scenario`, `--no-open` | Fake app and demo trace |
| `request-xray traces` | List recent traces | `--service`, `--since`, `--error`, `--limit`, `--json` | Summaries |
| `request-xray inspect <trace-id>` | Inspect a trace | `--tree`, `--json` | Trace + spans |
| `request-xray compare` | Compare traces (v0.4+) | `--baseline`, `--candidate`, `--json` | Measured differences and caveats |
| `request-xray explain <trace-id>` | Evidence summary (v0.5+) | `--offline`, `--provider` | Facts and hypotheses |
| `request-xray export` | Redacted export (v0.4+) | `--trace`, `--format`, `--out` | Local file |
| `request-xray test` | CI performance budgets (v0.4+) | `--budget`, `--json` | Pass/fail and exit code |
| `request-xray config validate` | Check configuration | `--file` | Schema errors/normalized config |

### Command details

#### `init`

Must **not** overwrite user files without confirmation. Dry-run shows proposed changes. Detect ESM/CJS, framework type and scripts, but never assume instrumentation order is correct solely based on file names. Generate `instrumentation.mjs` or a TypeScript entrypoint and an example script. Print manual instructions where automation cannot safely modify setup.

#### `dev`

Starts one native Rust binary with bounded listeners and DB. Prints actual dashboard URL; must not call it an OTLP endpoint. On SIGINT/SIGTERM, stop accepting new requests, drain within timeout, flush storage, close DB, and exit. Handle stale PID/ports, process crash, and config errors.

#### `doctor`

Checks: platform binary found, config valid, local port reachable, OTLP endpoint path/protocol, SQLite writable, collector API healthy, app exporter parameters, environment/SDK conflicts, loopback binding, and possible lack of startup preload. When evidence is insufficient, show warning rather than false success.

#### `traces` and `inspect`

Text and JSON output must be bounded, stable and scriptable. Preserve nanoseconds as strings in JSON. `inspect` should distinguish tree edges and span links and mark orphans. Validate IDs before database access.

#### `compare` and `test`

For initial versions, hidden or return an explicit "not supported in this release" error. Never output invented benchmark results. Later budget checks must state sample count, environment, uncertainty, and baseline selection.

### Proposed CLI exit codes

| Exit code | Meaning |
|---|---|
| `0` | Successful operation |
| `1` | General runtime or uncategorized failure |
| `2` | Invalid usage/configuration |
| `3` | Storage or collector unavailable |
| `4` | Unsupported platform or missing native asset |
| `5` | Performance budget failed (future) |

Avoid changing documented codes without a major release or explicit compatibility policy.

## 7. Node SDK API

The [Node SDK guide](NODE_SDK.md) documents the workspace SDK, CommonJS and ESM startup, privacy defaults, manual spans, and use with an existing OpenTelemetry provider.

## 8. User interface walkthrough (planned)

### Screen A: Setup / connection status

Displays collector listening, SDK exporter activity, observed services, dropped/rejected spans, and next steps when no telemetry exists. Sample/demo telemetry is visibly labeled.

### Screen B: Trace explorer

Columns: start time, route/operation, duration, status, service count, observed span count, completeness. Filters: service, route, duration, time, errors, trace ID. Pagination must not fetch all spans at once.

### Screen C: Trace detail

Hierarchy on left; waterfall center; inspector side panel. Click a span to view timestamps, safe attributes, events, links and errors. Color is supplemental to textual status. Show missing parents, delayed spans, uninstrumented gaps, and clock uncertainty.

### Screen D: Service graph (v0.2+)

Directed graph of observed relationships. Visual edges mean **at least one observed call/link**, not proof of a complete architecture. Edges can be grouped by count, latency, and error ratios when adequately sampled.

### Screen E: Investigations (v0.2+)

Rule cards link to exact supporting span IDs and describe limitations. Example: "100 similar SQL calls in a single trace; possible N+1." User can inspect the queries and decide whether batch loading is appropriate.

### Screen F: AI assistant (v0.5+)

Disabled by default; explains what sanitized evidence would be sent. Results distinguish observed facts from hypotheses and cite span IDs. Never auto-edit code, make remote writes, or send raw credentials.

## 9. HTTP API reference

The read-only API listens at `http://127.0.0.1:4731/api/v1` by default. The [query API guide](QUERY_API.md) documents every route, cursor, filter, precision rule, error, and generated TypeScript client.

### OTLP ingestion endpoint

```http
POST http://127.0.0.1:4318/v1/traces
Content-Type: application/x-protobuf
Content-Encoding: gzip  # optional

<binary ExportTraceServiceRequest>
```

**Do not send plain JSON to the protobuf endpoint expecting it to work.** Use official exporter libraries or a correctly serialized proto fixture. The collector must implement the prescribed OTLP export response and error/partial-success handling.

## 10. How each major subsystem executes

### Application request pipeline

```text
Client request -> application receives request
  -> OTel SDK starts server span
  -> SDK instruments supported HTTP/DB/cache calls
  -> context propagates to downstream services
  -> spans end, batched exporter serializes OTLP protobuf
  -> exporter POSTs to configured collector
```

### Collector pipeline

```text
Axum POST /v1/traces
  -> method/media/encoding validation
  -> compressed + decoded size limits
  -> protobuf decode -> canonical mapping
  -> privacy filter and limits
  -> transactional storage via SQLx/SQLite
  -> OTLP response / rejection accounting
```

### Query and dashboard pipeline

```text
Browser -> React/TanStack Query
  -> GET /api/v1/traces
  -> Rust API validates filters
  -> parameterized SQL query
  -> sanitized JSON DTO (nanosecond strings)
  -> waterfall layout, gaps and inspector
```

### Analysis pipeline (post-MVP)

```text
Trace spans/events/links -> normalized time graph
  -> deterministic measured signals
  -> rule evaluation + references + limitations
  -> analysis JSON -> UI evidence cards
  -> optional explicitly-enabled AI explanation
```

## 11. Troubleshooting matrix

| Symptom | Likely explanation | Checks and actions |
|---|---|---|
| No traces appear | SDK not started, wrong port, exporter batch not flushed | Run `doctor`, verify local listener and full `/v1/traces` endpoint |
| HTTP calls not instrumented | Framework imported before SDK, incompatible module format/loader | Use verified startup preload and ESM-specific hook guidance |
| Duplicate spans | Two SDKs/instrumentations applied | Detect provider conflicts, disable redundant instrumentation |
| Service appears unnamed | Missing `service.name` resource | Configure `serviceName` or `OTEL_SERVICE_NAME` |
| Trace split across services | Context not propagated or sampled | Check `traceparent`, baggage policies, proxies, queue headers |
| Database query missing | Client/driver not supported or auto-instrumentation omitted | Check SDK plugin, driver version, and manual spans |
| Query attributes absent | Privacy redaction or SDK doesn't emit them | Check redaction counters; do not disable safe defaults casually |
| Spans arrive late | Export batching, async consumers, network delays | Check last seen and completeness indicator |
| Time bars look inconsistent | Skewed host clocks or malformed timestamps | Check NTP/clock diagnostics, rely on per-span durations |
| High collector memory | Unbounded input or pathological trace | Check ingestion caps, queue limit, batch size and retention |
| SQLite is locked | Too many competing writers or long transactions | Inspect WAL, writer serialization and busy timeout |
| npm command not found | CLI not installed/published or platform asset missing | Confirm package release status and supported target; check `npm view` only after publishing |
| Collector works but dashboard doesn't | Different query vs OTLP ports | OTLP on `4318`; UI/API on `4731` by default |
| HTTP 415 on OTLP | Wrong content type | Use protobuf exporter with `application/x-protobuf` |
| Unexpected secrets in trace | Bad instrumentation attrs/redaction missing | Stop collection, remove compromised data, fix policy and regression test |
| AI answers confidently without evidence | Model hallucination/prompt injection | Disable AI, rely on deterministic rules and source span references |

## 12. Testing guide for contributors

### Rust

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p xray-otlp
cargo test -p xray-storage
```

### TypeScript and dashboard

```bash
pnpm -r lint
pnpm -r typecheck
pnpm -r test
pnpm --filter @request-xray/dashboard build
```

Package filter names are placeholders until actual manifests exist. `pnpm -r` scripts must be created in each relevant package. Avoid marking a command green because it skipped all projects.

### End to end

1. Start clean local Rust server in isolated temp directory.
2. Start actual OTel-instrumented Express test process.
3. Call test endpoint and wait for verified exporter flush/ingest.
4. Query trace API by returned/recorded trace ID; assert span identity and parent references.
5. Open UI via Playwright and assert span is visible and time alignment correct.
6. Stop SDK/server; verify exit, SQLite restart persistence and no leaked processes.
7. Repeat with parent missing, async late span, upstream failure, gzip, and oversized payload.

Do not use random fixed sleeps as the sole synchronization mechanism in E2E tests. Poll with deadlines and explicit diagnostics.

### Performance benchmarking

Measure collector CPU/RSS, accepted spans/sec, p95 ingestion latency, SQLite growth, worst-case trace query latency, UI responsiveness with 10k spans, and Node SDK overhead. Publish exact hardware, OS, Node/Rust versions, payload shape, span count, sampling, and confidence intervals where useful. No unverified “low overhead” or “zero overhead” claims.

## 13. Security operations

- Default on-disk storage directory `.request-xray/` is gitignored; permissions private to the current user when possible.
- Retention deletes complete traces transactionally and reports counts.
- Local dashboard does not open to LAN; reject unsafe bind requests unless future authenticated remote mode is explicitly supported.
- Never store response bodies, credentials or payment records by default; SDK and collector both enforce safe capture rules.
- AI integration, external export, and remote analytics are all opt-in.
- Trace data from user-controlled services should be considered untrusted input. Apply escaping, size caps, and sanitization.
- Provide a security disclosure path in `SECURITY.md` before first publication.

## 14. npm publishing and maintainership (future)

### Package intent

- `request-xray`: CLI, resolving precompiled Rust native executable from platform-qualified npm artifacts.
- `@request-xray/node`: TS SDK and OTel bootstrap.
- `@request-xray/core`: optional napi-rs addon **only when justified**, not an automatic prerequisite.

### Release preparation checklist

```bash
# Future scripts / command sequence to verify; do not execute without scaffolding
cargo test --workspace
pnpm -r lint
pnpm -r typecheck
pnpm -r test
pnpm -r build
npm pack --dry-run
```

Platform release CI builds signed/attested archives or platform npm packages, verifies checksums, does clean install smoke tests on supported OS targets, and publishes platform packages before the root CLI. Multi-package registry publication is **not atomic**; use staged verification, pinned exact versions, rollback/recovery plan, and release checklist. Use npm trusted publishing where available and maintain token security. Never publish from an unreviewed agent run.

### Version policy

- Pre-1.0 may change APIs, with changelogs and clear migration notes.
- SemVer after 1.0 with compatibility tests for CLI, TypeScript API, HTTP API, and stored DB schema.
- Experimental feature names must be clearly namespaced or capability-flagged.
- Support policy and EOL of Node/Rust/platform targets must be published.

## 15. Worked example: tracing ShopFlow checkout

Assumptions: all services emit interoperable OTel spans; the payment outcome is **not** inferred without explicit business instrumentation.

1. Engineer enables Node server tracing in NestJS service.
2. Java order service is configured with its own OTel agent/exporter pointed to Request X-Ray.
3. Outbound HTTP propagates W3C context. Redis/Postgres operations create child spans if supported.
4. Payment service emits a `CLIENT` span for provider request; application optionally adds safe business events like `payment.capture_confirmed`.
5. Kafka producer and consumer emit linked spans when propagation is configured; missing consumer span is reported as unknown, not failed delivery.
6. After user request, React explorer lists trace ID, start time, elapsed wall time and error status.
7. Span waterfall shows 3.9-second provider call dominating elapsed request time.
8. Evidence rule reports observed retry attempts; if idempotency metadata absent, it says “idempotency unknown.”
9. Engineer compares representative pre/post-deploy traces only after enough matching data is available.
10. A future optional assistant suggests diagnostics; it cannot charge/refund/replay customer transactions.

## 16. Documentation structure as the project grows

Maintain this file as the canonical end-to-end development reference for the first release. After Step 7 split topic guides into `docs/getting-started.md`, `docs/cli.md`, `docs/sdk-node.md`, `docs/otlp.md`, `docs/ui.md`, `docs/security.md`, `docs/troubleshooting.md`, `docs/contributing.md`, `docs/release.md` and `docs/adr/`. Keep `DOCUMENTATION.md` as the landing page with links and version badges. Generate API docs from actual schemas/code and treat them as source-controlled build artifacts where appropriate.

## 17. References / compatibility notes

The official standards, not this speculative document, determine protocol and semantic conventions:

- OTLP spec: https://opentelemetry.io/docs/specs/otlp/
- OTel JS: https://opentelemetry.io/docs/languages/js/
- Node instrumentation startup: https://opentelemetry.io/docs/languages/js/getting-started/nodejs/
- JS ESM support: https://github.com/open-telemetry/opentelemetry-js/blob/main/doc/esm-support.md
- HTTP semantic conventions: https://opentelemetry.io/docs/specs/semconv/http/
- Database semantic conventions: https://opentelemetry.io/docs/specs/semconv/db/database-spans/
- Messaging semantic conventions (still evolving): https://opentelemetry.io/docs/specs/semconv/messaging/
- napi-rs publishing: https://napi.rs/docs/deep-dive/release

**Maintenance rule:** Check versions and compatibility during each release; do not treat a linked upstream specification as permanently frozen. Maintain a semconv normalization strategy with fixtures for legacy and current emitted attributes.
