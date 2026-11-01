# Integration verification matrix

Use this as the gate for Request X-Ray milestones. A green HTTP response does **not** establish that a trace exists. Verify trace IDs, parent/child relationships, service names, recorded status, and latency in the collector/query API or exported OTLP payload.

| Area | Run | Required evidence | Layer | Status before SDK/collector |
|---|---|---|---|---|
| Basic HTTP | `scenario:all` fast | Server span and route attributes | Auto HTTP | Unverified |
| Cross-service | checkout | Same trace ID across two processes | Propagation | Unverified |
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
| Privacy | privacy | No request/response body captured by default | Security | Unverified |
| Service query | checkout | Search filter shows both services | Storage/query | Unverified |
| Trace comparison | fast then slow | Detect increased latency | Analysis | Not implemented |
| Collector outage | stop Rust collector | App still serves requests | Failure isolation | Unverified |
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
