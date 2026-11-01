# Request X-Ray

> **Project status: specification / pre-implementation.** This repository describes a proposed open-source product. Commands, package names, screenshots, API shapes, and version milestones below are **design targets**, not evidence of a published or working release.

**Request X-Ray** is a Rust-powered, local-first request investigation platform distributed through **npm**. It turns OpenTelemetry traces into a usable debugging experience: request waterfalls, service relationships, failure propagation, query-pattern analysis, trace comparisons, and evidence-backed explanations.

**Tagline:** *Chrome DevTools for the journey of your backend request.*

## Why

A checkout request can touch a browser, API gateway, authentication service, database, cache, payment provider, and Kafka consumer. Logs in different places rarely tell a coherent story. Request X-Ray connects instrumented operations into a navigable trace and helps answer:

- What happened to this request, and in what order?
- Which operation dominated latency, and what happened in parallel?
- Did the error originate here or propagate from downstream?
- What changed between a fast trace and a slow one?
- Where is visibility missing because a service was not instrumented?
- What evidence should be collected next before claiming root cause?

## Principles

1. **OpenTelemetry first.** Reuse OTLP and W3C trace context; no proprietary trace protocol.
2. **Rust where it matters.** Native collector, storage, aggregation, analysis, and CLI; existing JS instrumentation in Node processes.
3. **Local first.** First useful trace without Docker, SaaS signup, account, or paid AI.
4. **Honest inference.** Distinguish observed facts, detected patterns, hypotheses, and unknowns.
5. **Safe by default.** Never collect request/response bodies, passwords, cookies, auth headers, payment details, or raw SQL bind values by default.
6. **Non-disruptive.** Telemetry failure must not break the observed application.
7. **Friendly to contributors.** Small crates, explicit interfaces, fixtures, contract tests, public architectural decisions.

## Real-life example

A ShopFlow customer clicks **Place Order**. A request takes 5.4 seconds and ends with a 504. Request X-Ray shows an instrumented HTTP chain through NestJS → Spring Boot → payment service and highlights a 3.9-second outbound payment call. It also identifies retry spans and a DB rollback event when that metadata exists. The tool reports **observed evidence**, then suggests checking the payment provider, timeout configuration, idempotency, and order/payment reconciliation. It does **not** claim that a payment was captured unless the application emitted a trusted business event.

```text
POST /api/checkout                         5,420 ms  ERROR
  api-gateway: validate session                35 ms  OK
  redis: GET session                           18 ms  OK
  orders: create order                        110 ms  OK
  payments: charge                          4,100 ms  ERROR
    payment-provider: POST /charge          3,900 ms  TIMEOUT
  orders: rollback                             47 ms  OK
  [gap] downstream instrumentation missing       ?
```

## Intended feature map

Legend: **P0** first useful release; **P1** next; **P2** advanced; **R** research/conditional.

| Capability | Priority | Notes |
|---|---|---|
| OTLP/HTTP protobuf trace ingestion | P0 | Interoperable ingestion; explicit errors/partial success |
| Node HTTP auto-instrumentation | P0 | Supported modules, initialized before app imports |
| Trace list, details, waterfall, span inspector | P0 | Time precision preserved, missing data explained |
| Rust CLI, local server, SQLite | P0 | No Docker required |
| Search by trace ID, service, route, error, duration | P0 | Indexed local queries |
| Health/doctor commands, sample app, docs | P0 | Diagnose integration problems |
| PostgreSQL/Redis instrumentation examples | P1 | OTel integrations; safe attributes |
| Service graph and cross-service context health | P1 | Multiple languages via OTLP |
| Structured error grouping, retries, timeout attribution | P1 | Evidence-led classification |
| N+1/query-shape heuristics | P1 | Suspicions, not automatic proof |
| Trace comparison, percentiles, critical path | P1 | Statistical guardrails |
| Kafka and RabbitMQ producer/consumer links | P1 | Messaging semantics are evolving |
| Export/import, shareable redacted reports | P1 | Explicit export controls |
| Node event-loop and CPU profile correlation | P2 | Separate telemetry source required |
| Performance budgets and CI integrations | P2 | Adequate sample size required |
| VS Code source navigation and Git/deploy metadata | P2 | Only when source/version mapping exists |
| Optional AI trace explanation | P2 | Opt-in provider, redacted evidence only |
| Trace-derived tests, safe local replay, fault injection | R | Never replay side effects against production |
| Authenticated remote/team mode, advanced storage | R | Separate threat model and deployment mode |
| Browser trace UI + end-user navigation | R | Browser OTel instrumentation has distinct maturity constraints |

For the complete specifications see [ARCHITECTURE.md](docs/ARCHITECTURE.md), [ROADMAP.md](docs/ROADMAP.md), and [DOCUMENTATION.md](docs/DOCUMENTATION.md).

## Planned npm experience (NOT YET AVAILABLE)

```bash
# Future published package identifiers, subject to availability
npm install @request-xray/node
npx request-xray init
npx request-xray dev
npx request-xray doctor
```

Minimal intended app instrumentation:

```ts
// instrumentation.ts — illustrative, not executable until SDK exists
import { startXRay } from '@request-xray/node';

await startXRay({
  serviceName: 'orders-api',
  collectorEndpoint: 'http://127.0.0.1:4318/v1/traces',
  capture: { http: true },
});
```

**Initialization order matters:** instrument **before** importing the HTTP server/framework. For Node.js, startup preload patterns and ESM loader behavior vary; follow [DOCUMENTATION.md](docs/DOCUMENTATION.md) and version-specific OpenTelemetry docs. Running `init` may generate a preload file but cannot guarantee instrumentation of an application already loaded.

### Planned local ports

- `127.0.0.1:4318`: OTLP/HTTP collector, `POST /v1/traces` (not the dashboard).
- `127.0.0.1:4731`: dashboard and read-only query API (configurable).
- OTLP/gRPC `4317`: **not supported in v0.1**, planned later.

### Proposed package layout

| Package | Role |
|---|---|
| `request-xray` | npm-distributed CLI wrapper that launches the Rust binary |
| `@request-xray/node` | TypeScript Node/OpenTelemetry bootstrap and helpers |
| `@request-xray/core` | **Optional later** napi-rs addon for measured hotspots |

The CLI's prebuilt binary distribution will be an explicit release decision; it need not use napi-rs. Runtime binaries and native addon packages are different things. No compiler or arbitrary network installer should be required for supported platforms after installation.

## Technology stack

- **Rust:** Tokio, Axum, Clap, Serde, Prost/prost-build, SQLx + SQLite, tracing, criterion, proptest (where useful).
- **Node / npm:** TypeScript, OpenTelemetry JS SDK + instrumentations, pnpm workspaces, Vitest.
- **Dashboard:** React, Vite, TypeScript, TanStack Query; SVG/Canvas timeline (performance-dependent), React Flow for topology.
- **Quality:** cargo fmt/clippy/test, rustdoc, ESLint, TypeScript strict, Playwright, GitHub Actions, dependency/security audits.
- **Interoperability:** OTLP/HTTP protobuf initially; W3C trace context; optionally Jaeger/Tempo through the existing OTel ecosystem.

## Performance and reliability goals (targets, not benchmarks)

- Host application should remain operational if X-Ray collection stops.
- Bound ingestion queue, payload sizes, memory, storage size, and retention.
- Define reproducible benchmark fixtures, CPU/memory envelopes, and sampling limits before making overhead claims.
- Preserve original nanosecond timestamps, distinguish wall-clock from monotonic durations, and avoid attributing CPU time from wall time.

## Example directory structure (target)

```text
request-xray/
├── crates/
│   ├── xray-core/            # Canonical domain models, validation
│   ├── xray-otlp/            # OTLP decoding and conversion
│   ├── xray-collector/       # Axum ingestion, limits, backpressure
│   ├── xray-storage/         # SQLite repositories and migrations
│   ├── xray-analysis/        # Graph, critical path, rules, regressions
│   ├── xray-api/             # Dashboard query API
│   └── xray-cli/             # Rust command line entry
├── packages/
│   ├── cli/                  # npm launcher / platform distribution
│   ├── node/                 # TS instrumentation, optional helpers
│   └── dashboard/            # React app
├── examples/
│   ├── express-basic/
│   └── distributed-checkout/
├── tests/fixtures/otlp/
├── docs/                     # Future topic guides
├── .github/workflows/
├── Cargo.toml
├── pnpm-workspace.yaml
├── README.md
├── docs/ARCHITECTURE.md
├── docs/ROADMAP.md
└── docs/DOCUMENTATION.md
```

## Contributing and governance (to establish in Step 0)

- Apache-2.0 recommended, pending dependency and name checks; do not claim a license until `LICENSE` exists.
- Add `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`, pull request templates, issue labels, and release notes.
- Public issues should separate bugs, instrumentation support, analysis rules, UI improvements, and security.
- Any new telemetry attribute must specify privacy classification and default handling.
- Any analysis rule must have positive/negative fixtures and explainable evidence.
- Community contributions must pass CI and avoid silent protocol breaking changes.

## Current status

**Specification only.** No npm package, collector, dashboard, or tests are claimed to exist yet. Start with [ROADMAP.md](docs/ROADMAP.md) Step 0, and do not check off a task without proof from executable tests or generated artifacts.

## Authoritative specifications

- [OTLP Specification](https://opentelemetry.io/docs/specs/otlp/)
- [OpenTelemetry JavaScript](https://opentelemetry.io/docs/languages/js/)
- [OpenTelemetry HTTP semantic conventions](https://opentelemetry.io/docs/specs/semconv/http/)
- [OpenTelemetry database semantic conventions](https://opentelemetry.io/docs/specs/semconv/db/database-spans/)
- [OpenTelemetry messaging semantic conventions](https://opentelemetry.io/docs/specs/semconv/messaging/)
- [napi-rs distribution docs](https://napi.rs/docs/introduction/getting-started)

## License

**Planned:** Apache-2.0. Add the license file and copyright notices during Step 0.
