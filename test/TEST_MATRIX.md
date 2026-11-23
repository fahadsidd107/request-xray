# Integration verification matrix

Use this as the gate for Request X-Ray milestones. A green HTTP response does **not** establish that a trace exists. Verify trace IDs, parent/child relationships, service names, recorded status, and latency in the collector/query API or exported OTLP payload.

| Area | Run | Required evidence | Layer | Status |
|---|---|---|---|---|
| Basic HTTP | `scenario:all` fast | Server span and route attributes | Auto HTTP | Unverified |
| Cross-service | SDK integration test | Express W3C parent IDs agree across processes. NestJS checkout persists one trace spanning both services. | Propagation | Verified |
| Concurrent fanout | parallel | Three downstream client/server pairs overlap | Trace correlation | Unverified |
| Sequential calls | sequential | Calls ordered in time | Waterfall | Unverified |
| Slow operation | slow | Named span ≈400ms | Custom spans | Unverified |
| Exception | error | 500 + errored span | Errors | Unverified |
| Timeout | timeout | 504 + timeout metadata | Error propagation | Unverified |
| Retry | retry | 503, 503, 200 with attempt attributes | Retry analysis | Unverified |
| Simulated SQL | nplusone | Repeated mock DB spans labeled mock | Pattern UI only | Unverified |
| Simulated Redis | cache | Mock cache miss and DB fallthrough | Pattern UI only | Unverified |
| Simulated queue | queue | Mock publish/consume spans | Span UI only | Unverified |
| CPU | cpu | HTTP response; optionally event-loop metric | Profiling | Unverified |
| Privacy | SDK integration test | Synthetic header, body and query values are absent from emitted protobuf. | Security | Verified |
| Service query | SDK integration test | Filtered checkout trace reports two services after SDK flush. | Storage/query | Verified |
| Trace comparison | fast then slow | Detect increased latency | Analysis | Not implemented |
| Collector outage | SDK integration test | Express still serves requests after the collector stops. | Failure isolation | Verified |
| Native binary | `start:xray` | Runtime loads actual compiled package | Packaging | Not implemented |
| Real PostgreSQL | Future fixture | DB client auto-spans and real query | Driver integration | Not in this lab |
| Real Redis | Future fixture | Real cache client spans | Driver integration | Not in this lab |
| Real Kafka | Future fixture | Producer/consumer span links | Message tracing | Not in this lab |
| Cross-language | Future fixture | Java/Go trace propagation | Interoperability | Not in this lab |
| Load/sampling | Future fixture | Drop rate + consistent sampling evidence | Reliability | Not in this lab |
| AI / replay / CI | Future fixture | Purpose-built assertions | Future modules | Not implemented |

## Automated assertions

`npm run verify` checks TypeScript compilation and HTTP behavior with `node:test`. The smoke suite starts both servers itself and does not require a pre-running collector. It does not validate OTLP or dashboard functionality. Add Rust collector contract tests to the parent workspace as those APIs land.

## Recommended extension order

1. Implement collector ingestion and query API; write a trace assertion harness that checks spans from `start:otel`.
2. Link the actual SDK and rerun the same assertions under `start:xray`.
3. Add optional Docker-backed PostgreSQL and Redis fixtures with real drivers.
4. Add Kafka/RabbitMQ and a separate language service.
5. Add production-shaped sampling, concurrency, resilience, and redaction tests, isolated from the smoke suite.
