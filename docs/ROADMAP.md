# Request X-Ray — Implementation Roadmap

**Status:** Steps 1.1–1.7 verified on 2026-11-19. Step 0 still has open gates. See the [foundation audit](STEP0_AUDIT.md) for verification results and blockers. **A checkbox means code + tests + documented proof**, not merely files created.

## Operating instructions for contributors / Codex

Read `README.md`, `docs/ARCHITECTURE.md`, this file, and `docs/DOCUMENTATION.md` before starting. Implement the **first unfinished task** unless directed to a specific step. Never claim unpublished packages or nonexistent commands work. Do not change public contracts without updating docs and contract tests. Maintain a short progress entry in the PR or a future `docs/progress.md`: task ID, changed paths, commands run, test results, remaining limitations. Always use reproducible demo data and offline/local tests. Do not publish npm packages without explicit owner approval.

### Definition of done for every subtask

- Code compiles and passes the relevant test(s), including negative cases.
- `cargo fmt --check`, relevant `cargo clippy -D warnings`, `cargo test`, and TS lint/typecheck/tests pass once those packages exist.
- Interfaces, errors, security checks, and user-facing behavior documented.
- No new unbounded allocations or unredacted sensitive fields.
- A verification command and expected output are provided.
- Checkbox updated **only after verification**; no fabricated results.

### Release gates

- **Milestone M0:** Build scaffold with working Rust and TS CI.
- **Milestone M1:** Real OTLP spans travel into Rust, SQLite, and the trace API.
- **Milestone M2 / v0.1.0 candidate:** npm CLI + SDK + local dashboard usable end to end, with platform binary smoke tests.
- **Milestone M3 / v0.2.0:** database/cache intelligence, filtering, errors, and service overview.
- **Milestone M4 / v0.3.0:** reliable cross-service and async tracing documentation/examples.
- **Milestone M5 / v0.4.0:** explainable analysis and regression comparisons.
- **Milestone M6 / v0.5.0:** optional AI and developer tools.
- **Milestone M7 / v1.0.0:** stability, security, compatibility, and production-readiness review.

---

## STEP 0 — Repository and product decisions [M0]

**Outcome:** Someone can clone the repo, understand the product, and start the dev environment without guessing.

- [ ] **0.1 Name and ownership:** Check availability of `request-xray` and `@request-xray/*` npm identifiers, repository name, trademark collision risk, and domain if desired. Record final names; rename examples if needed.
- [ ] **0.2 License and governance:** Add Apache-2.0 `LICENSE` only after decision; `CODE_OF_CONDUCT.md`, `CONTRIBUTING.md`, `SECURITY.md`, issue/PR templates, maintainer policy and release policy.
- [ ] **0.3 Toolchain:** Pin supported Rust toolchain/MSRV, Node LTS support, pnpm version via corepack/packageManager, version policy, `.editorconfig`, `.gitignore`, formatting.
- [ ] **0.4 Root scaffold:** Rust Cargo workspace, pnpm workspace, `crates/`, `packages/`, `examples/`, `tests/fixtures/`, `docs/adr/` with minimal valid manifests.
- [ ] **0.5 CI baseline:** GitHub Actions fmt, clippy, unit tests, JS lint/typecheck/tests, dependency audits and basic secret scanning, caching.
- [ ] **0.6 ADRs:** Write ADR-0001 to 0007 listed in `ARCHITECTURE.md`; clearly decide OTLP ACK semantics and binary distribution before implementation.
- [ ] **0.7 Contract style:** OpenAPI source-of-truth, semantic versioning standards. commit/changelog standards, API error envelope, typed config spec.
- [ ] **0.8 Documentation integrity:** README states pre-release accurately; all examples flagged planned; no fake shields/download counts.

**Verify:** fresh `git clone`, `cargo test --workspace`, `pnpm install --frozen-lockfile`, `pnpm -r typecheck`, `pnpm -r test`; all relevant checks run and documented. *Commands become valid only after workspace scripts are created.*

**Exit criterion:** Repository scaffold, conventions, CI and ADRs are reviewable and green.

---

## STEP 1 — Domain model and protobuf compatibility [M1]

- [x] **1.1 Pin OTLP protobuf spec:** Choose official proto revision and generation tooling; document source/hash, license and update policy. See [provenance and verification](OTLP_PROTOBUF.md).
- [x] **1.2 `xray-core`:** Newtypes for TraceId (16 bytes), SpanId (8 bytes), SpanKind, SpanStatus, resource, scope, events, links, attributes; strict lossless conversions. See [domain model verification](ARCHITECTURE.md#step-12-domain-model-verification).
- [x] **1.3 Timestamp semantics:** Preserve nanoseconds as `i64` where safe, validate range and ordering; test boundary values and JS JSON serialization. See [clock semantics](ARCHITECTURE.md#clock-semantics).
- [x] **1.4 `xray-otlp`:** Decode `ExportTraceServiceRequest` protobuf; convert resource/scope/span/event/link hierarchy into canonical models without dropping necessary fields. See [trace decoding](OTLP_PROTOBUF.md#trace-decoding-and-conversion).
- [x] **1.5 Test fixtures:** Capture genuine exporter payloads for one span, nested parallel spans across services, non-ASCII attributes, events, links, errors and large fields. Derive malformed ID and invalid timestamp cases from a capture. See [exporter fixtures](../tests/fixtures/otlp/README.md).
- [x] **1.6 Conversion properties:** Tests for valid IDs, duplicate keys, dropped-count propagation, scope schema URLs, status, and attributes. See [property and fixture verification](OTLP_PROTOBUF.md#exporter-fixtures-and-verification).
- [x] **1.7 Explicit errors:** Document categorized errors (`invalid_payload`, `resource_limit`, `invalid_span`, `storage_unavailable`) and rejected-span tally. See [decoder error handling](OTLP_PROTOBUF.md#trace-decoding-and-conversion).

**Verify:** `cargo test -p xray-core -p xray-otlp`; a verified OTLP fixture yields expected trace/span relationships and retains all agreed fields.

**Exit criterion:** Interoperable wire data has a validated internal representation.

---

## STEP 2 — Rust collector and ingestion safety [M1]

- [ ] **2.1 Axum server:** `xray-collector`, config-driven OTLP listener `127.0.0.1:4318`, health endpoint on query listener later.
- [ ] **2.2 OTLP POST:** `/v1/traces` with protobuf, correct content type, appropriate OTLP success response and rejection semantics.
- [ ] **2.3 Gzip:** Support `Content-Encoding: gzip` and strict compressed/decompressed caps; reject unsupported encodings.
- [ ] **2.4 Security:** Local bind only, request size ceiling, attribute/event/span caps, strict decoding; avoid body/secret logs.
- [ ] **2.5 Redaction:** Default denylist and per-attribute allow/deny config; strip risky URL query strings, auth/cookies, token-like fields before storage.
- [ ] **2.6 Backpressure:** Bounded ingestion concurrency/queue, admission rejection, accounting and tests; no unbounded `tokio::spawn` per payload.
- [ ] **2.7 ACK semantics:** Implement ADR policy; preferred initial ACK after successful SQLite commit (integration in Step 3), no false positives.
- [ ] **2.8 Counters/diagnostics:** Accepted/rejected/redacted/duplicate spans, size errors, write failures, source endpoint; no PII.
- [ ] **2.9 Negative tests:** Invalid protobuf, wrong media type, oversize/gzip bomb, queue full, timeout, collector shutdown, no side effects after rejection.

**Verify:** real OTel exporter POSTs protobuf to `/v1/traces`; invalid payload returns documented error; gzip works; collector remains stable under bounded stress.

**Exit criterion:** Secure, standards-compatible ingestion endpoint, pending durable persistence wiring.

---

## STEP 3 — SQLite repository and trace lifecycle [M1]

- [ ] **3.1 SQLx migration:** `resources`, `traces`, `spans` with proper keys/indexes, events/links retained, referential integrity on.
- [ ] **3.2 Writer:** Transactional batches with stable deduplication and conflict diagnostics; ACK after commit according to ADR.
- [ ] **3.3 Read repository:** List traces, get trace, list ordered spans, filter by service/error/timestamps and cursor pagination.
- [ ] **3.4 Late spans:** Update trace summaries when additional spans arrive; preserve incomplete/missing-parent flags.
- [ ] **3.5 Retention:** Default configurable age/size limits, safe eviction and trace-level cascade behavior.
- [ ] **3.6 Storage resilience:** SQLite WAL, busy timeout, concurrent readers, restart and migration tests, disk full and write failures.
- [ ] **3.7 Duplicate semantics:** Identical span idempotency; conflicting content is counted and not silently overwritten.
- [ ] **3.8 End-to-end fixture:** OTLP → collector → DB → row assertions, including error, links, events, redaction, late spans.

**Verify:** inject known protobuf; stop/restart process; read persisted trace and exact span count/attributes; malformed payload never creates partial undisclosed rows.

**Exit criterion:** **M1:** Real trace data persistently reaches the Rust database with correctness guarantees.

---

## STEP 4 — Query API and OpenAPI [M2]

- [ ] **4.1 `xray-api`:** `GET /api/v1/health`, `/traces`, `/traces/{id}`, `/traces/{id}/spans`, `/services`, `/stats`, `/config/capabilities`.
- [ ] **4.2 Cursor pagination:** Stable deterministic ordering with indexes and hard max page size. Test concurrent insert behavior.
- [ ] **4.3 Search filters:** Trace ID, service, status/error, duration and time range, route when known; parameterized SQL.
- [ ] **4.4 JSON precision:** Nanosecond values as strings, error envelope, explicit nulls and availability flags.
- [ ] **4.5 OpenAPI 3.1:** Export schema; generate TS API client; schema/contract snapshots tested in CI.
- [ ] **4.6 Security:** Local bind, Host/Origin validation, no permissive CORS, secure HTTP headers, no unbounded result paths.
- [ ] **4.7 Incomplete trace model:** API exposes missing parent count, last seen time and observational completeness status.
- [ ] **4.8 Integration:** API reads only persisted real spans and never invents measurements or silently truncates sets.

**Verify:** integration tests for every route, filter, invalid ID, pagination, precision, SQL injection attempts, large spans, and missing traces.

**Exit criterion:** Typed, tested dashboard data contract.

---

## STEP 5 — TypeScript Node instrumentation SDK [M2]

- [ ] **5.1 SDK package:** `@request-xray/node`, strict TypeScript config, documented `startXRay` API, graceful shutdown.
- [ ] **5.2 OTel setup:** NodeSDK, batch exporter to OTLP/HTTP protobuf, service resource attributes, supported auto instrumentations.
- [ ] **5.3 Initialization:** CJS preload and ESM guidance; verify instrumentation runs before app imports. Avoid no-op SDK initialized too late.
- [ ] **5.4 HTTP spans:** Express example for inbound and fetch/HTTP outbound; route names and safe status metadata.
- [ ] **5.5 Context:** Trace propagation across two instrumented Node apps; W3C standard header behavior.
- [ ] **5.6 Safety:** Hide raw bodies/headers/URL query params by default; bounded exporter queue; no app-breaking collector failures.
- [ ] **5.7 Manual API:** `withSpan(name, fn)`, `addEvent(name, safeAttributes)`, optional active span; async context preserved.
- [ ] **5.8 Existing SDK coexistence:** Detect/configure provider conflicts and document integration with an existing OTel SDK.
- [ ] **5.9 Compatibility tests:** Node supported LTS versions, ESM/CJS, Express, Fastify or NestJS smoke fixture; flush/shutdown.

**Verify:** run sample `node --import ./instrumentation.mjs ./app.mjs`, issue HTTP request, see parent-child spans in SQLite. The real startup syntax depends on supported versions.

**Exit criterion:** JS apps can send traces with no custom vendor protocol.

---

## STEP 6 — Rust CLI + npm packaging [M2]

- [ ] **6.1 Rust Clap commands:** `serve`, `version`, `doctor`, `traces list`, `traces show`, `config validate` as internal/native primitives.
- [ ] **6.2 npm CLI shim:** `request-xray` package with `bin`, platform resolver, actionable error on missing/unsupported binary.
- [ ] **6.3 Platform packages:** Prebuilt macOS ARM64/x64, Linux x64/ARM64 glibc, Windows x64; executable permissions and integrity.
- [ ] **6.4 UX commands:** `npx request-xray init`, `dev`, `doctor`, `traces`, `inspect`, `demo`; only expose actually working commands in help.
- [ ] **6.5 Config:** Deterministic precedence CLI flags > env > project config > defaults; schema validation and secret redaction.
- [ ] **6.6 Process management:** `dev` starts Rust collector/API, serves UI, catches signals, cleans up, handles port collisions.
- [ ] **6.7 No unsafe installers:** No remote executable fetched on npm postinstall; package downloaded via normal npm distribution.
- [ ] **6.8 Tarball smoke tests:** `npm pack`, fresh temp install, CLI help/version/dev/doctor on supported platforms.

**Verify:** npm tarball installed into a clean project starts native Rust collector without a Rust toolchain.

**Exit criterion:** npm can deliver and run the Rust platform tool reliably.

---

## STEP 7 — Dashboard MVP [M2]

- [ ] **7.1 UI scaffold:** React/Vite/TypeScript, routing, theme and accessible components, API client generation.
- [ ] **7.2 Overview:** Trace count, collector status, observed services, failures; clear empty/setup/error states.
- [ ] **7.3 Trace table:** Stable pagination, filters, status badges, duration and service summaries.
- [ ] **7.4 Trace waterfall:** Hierarchical spans, relative offset/duration, overlapping concurrent operations, expandable rows, zoom/pan.
- [ ] **7.5 Span inspector:** Service, operation, kind, start/end, status, safe attributes, events, links, errors.
- [ ] **7.6 Missing data:** Orphans, dropped/unsampled spans, late updates, skew indicators; never fake a fully connected tree.
- [ ] **7.7 Performance:** 1k spans normal; benchmark a 10k-span trace with virtualization or canvas rendering if needed.
- [ ] **7.8 Accessibility:** Keyboard navigation, labeled controls, table semantics, high contrast, reduced motion.
- [ ] **7.9 Real E2E:** Run sample app, generate trace, verify exact IDs and timings appear in UI; demo mode clearly marked.

**Verify:** Playwright smoke test from installed npm package to browser waterfall; empty DB handled; error/missing-parent fixture visible.

**Exit criterion:** **M2** v0.1 candidate meets the actual one-install/one-request/one-trace experience.

---

## STEP 8 — v0.1 release engineering [M2]

- [ ] **8.1 Release checklist:** Versioning, changelog, `npm pack --dry-run`, license files, docs, provenance, binary checksums, SBOM.
- [ ] **8.2 Registry verification:** Check final package names, permissions, npm token/trusted publisher; all actions require owner approval.
- [ ] **8.3 CI matrix:** Build platform assets, install tarballs in clean runners, test official supported Node LTS versions.
- [ ] **8.4 Publish ordering:** Platform binaries first, root npm CLI package last, SDK package with compatible versions; handle partial publication retries.
- [ ] **8.5 Fresh-user docs:** Test README commands from zero on macOS ARM64 + Linux x64 at minimum; capture actual output.
- [ ] **8.6 Security test:** Local-only binding, redaction, no credential capture, bounded ingestion, npm package auditing.
- [ ] **8.7 Owner gate:** Only after explicit confirmation, tag, release, publish package(s), then verify `npm view` and fresh installation.

**Verify:** A new user can install, instrument Express, send a request, open dashboard, and see spans without building Rust.

---

## STEP 9 — v0.2 backend intelligence [M3]

- [ ] **9.1 PostgreSQL and ORM examples:** `pg`, Prisma, TypeORM; compatible semconv handling.
- [ ] **9.2 Redis integration:** Supported OTel instrumentation; optional explicit cache-hit business annotations.
- [ ] **9.3 SQL query-shape heuristic:** Normalize without preserving bind values, detect repeated per-request shapes and N+1 candidates.
- [ ] **9.4 Error taxonomy:** Recorded errors, timeouts, retries, upstream/downstream relationships, missing propagation warnings.
- [ ] **9.5 Analysis evidence format:** `observed|pattern|hypothesis`, source span references, rule version, limitations.
- [ ] **9.6 Service overview:** Observed calls by service/route, error rates and latency percentiles, with minimum-sample UI warnings.
- [ ] **9.7 Rules test suite:** Positive/negative SQL and timeout fixtures, sampling and missing-span false positives.
- [ ] **9.8 Product docs:** Performance interpretation, schema convention migrations, privacy.

**Verify:** Known N+1 fixture flagged; benign repeated queries not classified as confirmed bug; error trace points to supporting spans.

---

## STEP 10 — v0.3 distributed and async requests [M4]

- [ ] **10.1 Cross-service contexts:** W3C trace context through Node, Java Spring Boot and Go sample services using native OTel SDKs.
- [ ] **10.2 Kafka:** Producer/consumer spans and links, context headers; publish/consume under sampled/unsampled cases.
- [ ] **10.3 RabbitMQ or BullMQ sample:** Queue latency when trustworthy timestamps exist; dead-letter/retry event metadata.
- [ ] **10.4 Topology:** Aggregate observed service edges, link semantics, missing data caveats and update logic.
- [ ] **10.5 Propagation doctor:** Detect missing parents, mismatched trace IDs and uninstrumented suspected edges without false certainty.
- [ ] **10.6 Interop fixture:** ShopFlow checkout across TypeScript, Java, Go, Postgres, Redis, Kafka with deterministic dummy services.

**Verify:** Test request produces linked Node → Java/Go and producer-consumer traces; UI distinguishes sync nesting from async links.

---

## STEP 11 — v0.4 performance analysis and comparison [M5]

- [ ] **11.1 Span interval arithmetic:** Overlaps, exclusive duration using interval union, skew warnings.
- [ ] **11.2 Critical-path algorithm:** Define DAG semantics and validate with hand-calculated parallel span examples; mark uncertainty.
- [ ] **11.3 Baselines:** Store release/version/environment and selected time windows; no invalid cross-population comparisons.
- [ ] **11.4 Percentiles:** Accurate quantile method, sample counts, error bounds if approximate aggregation used.
- [ ] **11.5 Regression detector:** Guard against low samples, outliers, missing spans, sampling bias and deployment mix.
- [ ] **11.6 Compare UI:** Two traces and aggregate distributions, human-readable deltas, confidence/limitations.
- [ ] **11.7 Export:** Redacted OTLP/JSON/HTML reports with strict opt-in, bounded sizes and no unexpected egress.
- [ ] **11.8 Performance budgets:** CLI and CI check with stable benchmark fixture; define failure exit codes.

**Verify:** Parallel-span tests cannot double-count time; false regressions from mixed environments are rejected/warned.

---

## STEP 12 — v0.5 developer intelligence and optional AI [M6]

- [ ] **12.1 Rules-based explanations:** Work fully offline; facts, hypotheses and next diagnostic steps.
- [ ] **12.2 AI provider contract:** Opt-in only, preview of redacted payload, cost/token limits, timeout, local-model adapter if viable.
- [ ] **12.3 Trust controls:** Prompt-injection resistance for attributes/logs, no autonomous file/network changes, citations back to spans.
- [ ] **12.4 Git/source metadata:** Version/SHA capture, manual source mapping, VS Code deep links; no source claims without matching version.
- [ ] **12.5 Report generation:** Redacted incident summary, evidence links, markdown export.
- [ ] **12.6 Test hints:** Suggest regression tests using sanitized trace metadata; gated experiments for executable replay/fault injection.
- [ ] **12.7 Evaluation harness:** Known incidents with scored accuracy, unsupported-answer detection, privacy failures and cost measurements.

**Verify:** Offline analysis passes with AI disabled; AI never makes ungrounded claims of payment capture or root cause; no secrets leave the machine by default.

---

## STEP 13 — v1.0 hardening and expanded modes [M7]

- [ ] **13.1 API stability:** SemVer public contracts, migration policy, generated types, compatibility matrix.
- [ ] **13.2 Reliability:** Long-duration soak tests, disk full, crash recovery, partial ACK, delayed spans, collector restart, target load.
- [ ] **13.3 Performance:** Published reproducible results for overhead, span ingestion throughput, API p95 and UI memory; no fabricated performance metrics.
- [ ] **13.4 Security review:** Threat modeling, audit, disclosure workflow, dependency inventory, installation package scrutiny.
- [ ] **13.5 OTLP interoperability:** Real SDKs for Node, Java, Go, Python, gRPC support if feasible, semconv compatibility fixtures.
- [ ] **13.6 Accessibility + docs:** Keyboard/accessibility review, migration guides, troubleshooting matrix, contributor onboarding.
- [ ] **13.7 Production profile:** Only after TLS/auth, access control, retention, resource limits, sampling, remote exposure tests; do not label local unsecured mode production ready.
- [ ] **13.8 Version 1.0 owner gate:** Maintainers sign off with security + reliability + release reports.

**Verify:** Independent clean-machine installation, 24h+ soak test under defined workloads, audited security boundary, documented support policy.

---

## Backlog beyond 1.0 (not committed)

- [ ] Browser/user-action correlation with explicit consent and browser compatibility caveats.
- [ ] Advanced resource profiling/eBPF integrations (platform constraints; extra permissions).
- [ ] Profiles signal when ecosystem support stabilizes.
- [ ] Tail-based sampling with bounded buffers and loss accounting.
- [ ] Trace-derived safe integration test generation and deterministic mock replay.
- [ ] Controlled dev/test fault injection with strict production guards.
- [ ] ClickHouse/external backend adapters, Jaeger/Tempo import/export pathways.
- [ ] Authenticated team dashboard, workspace-sharing, OIDC, RBAC and tenant isolation.
- [ ] Kubernetes health correlation, deployments and automated analysis.
- [ ] Service-level objectives, alerts, incident timeline and notification integrations.
- [ ] Supported editor plugins (VS Code first) and diagnostics protocol.
- [ ] Plugin SDK with signed/permission-scoped extensions and compatibility tests.

## Working sequence for an AI coding agent

```text
1. Read all four top-level docs.
2. Identify first unchecked task (starting 0.1).
3. State intended files, constraints, tests, and acceptance criteria in the PR/task note.
4. Implement a minimal complete slice; add failure/negative tests.
5. Run exact local checks; document real pass/fail outcomes.
6. Update related docs and public examples.
7. Mark ONLY the verified task as complete.
8. Stop at the next task boundary; never skip ahead to cosmetics.
9. If blocked, leave unchecked and record exact blocker + proposed remedy.
10. Do not push/publish/package to remote registries without owner's explicit request.
```

## Cross-cutting release acceptance checklist

- [ ] Docs match actual implementation, with planned features marked planned.
- [ ] At least one real OTLP exporter works across process boundaries.
- [ ] Default capture excludes credentials, bodies, cookies, card data and raw bind values.
- [ ] Data gaps are visible; analysis never invents missing spans or causes.
- [ ] Clean npm install requires no Rust toolchain on supported OS targets.
- [ ] Performance and resource ceilings are configured/tested.
- [ ] New contributors can reproduce the demo without secret keys or paid accounts.
- [ ] Security and compatibility docs reflect supported versions and limitations.
