# Request X-Ray Integration Test Lab

A **private, disposable NestJS application** to place at `<request-xray-repo>/test/`. It is an integration fixture for a Rust-powered Request X-Ray collector, npm SDK, and dashboard. It contains two independently running NestJS HTTP servers, realistic test scenarios, named manual spans, an optional OpenTelemetry exporter, and Node's built-in smoke tests.

> **Status:** This fixture is runnable after `npm install`, but Request X-Ray itself is **not** bundled. `XRAY_MODE=otel` can emit traces to any running OTLP/HTTP collector. `XRAY_MODE=xray` requires the project's real `@request-xray/node` implementation. The fixture cannot certify package features that do not exist yet.

## Install in the repository

Copy the directory in this ZIP as `request-xray/test/`, with the main repository's `packages/` and `crates/` directories as siblings. The fixture has no absolute paths and does not alter the parent project.

```sh
cd request-xray/test
npm install
npm run build
npm start
# in a separate terminal:
npm run scenario:all
# in another terminal, optional:
npm run smoke
```

- Checkout service: `http://127.0.0.1:3900`
- Downstream payment/inventory service: `http://127.0.0.1:3901`
- `npm run verify` performs typechecking, compilation, and smoke tests.
- Node.js 20+ is required. The lab uses synthetic payments and in-memory simulations, never real money or credentials.

### Send traces to Rust collector

Start Request X-Ray's **implemented** OTLP receiver in the parent repo before starting the fixture. The intended endpoint in the documentation is `http://127.0.0.1:4318/v1/traces`.

```sh
cd request-xray/test
npm run build
npm run start:otel
# separate shell:
npm run scenario:all
```

Override the collector endpoint with `XRAY_COLLECTOR_URL=http://127.0.0.1:4318/v1/traces`. `otel` mode uses the standard OpenTelemetry JS SDK, independent of the future `@request-xray/node` package. A valid trace exported and visible in the Rust collector establishes **protocol compatibility**, not the package SDK's correctness.

### Test the actual npm SDK

After the real `@request-xray/node` package exists and is built, install/link it into this fixture, for example:

```sh
npm install --no-save ../packages/node
npm run build
npm run start:xray
```

The `xray` preload currently expects the *proposed* `startXRay(options)` export. It deliberately **fails loudly** if that SDK cannot be resolved or if its API is incompatible. Current adapter expects synchronous initialization so framework imports happen afterward; if the actual SDK has async initialization, revise the preload and startup mechanism rather than ignoring the Promise. Test both `start:otel` and `start:xray` independently.

## Scenarios

| CLI key | Route / behavior | Expected outcome | What it exercises |
|---|---|---|---|
| `fast` | GET `/scenarios/fast` | 200 | Baseline HTTP + custom span |
| `slow` | GET `/scenarios/slow?ms=400` | 200 | Long-running span |
| `error` | GET `/scenarios/error` | 500 | Exception capture |
| `validation` | GET `/scenarios/validation?count=-1` | 400 | Client error |
| `parallel` | GET `/scenarios/parallel` | 200 | Parallel downstream HTTP spans |
| `sequential` | GET `/scenarios/sequential` | 200 | Sequential downstream HTTP spans |
| `retry` | GET `/scenarios/retry` | 200 | Retry attempts with 503 responses |
| `timeout` | GET `/scenarios/timeout` | 504 | Downstream timeout and error span |
| `nplusone` | GET `/scenarios/n-plus-one` | 200 | Simulated repeated database spans |
| `cache` | GET `/scenarios/cache` | 200 | Simulated Redis hit/miss operations |
| `queue` | GET `/scenarios/queue` | 200 | **Simulated** message events (not Kafka) |
| `cpu` | GET `/scenarios/cpu` | 200 | Synchronous event-loop blocking |
| `privacy` | GET `/scenarios/privacy` | 200 | Minimal non-sensitive HTTP route |
| `notfound` | GET `/scenarios/not-found` | 404 | Explicit missing-resource behavior |
| `checkout` | POST `/orders/checkout` | 201 | Checkout → inventory → payment |
| `declined` | POST `/orders/checkout` | 502 | Downstream payment decline (402) |
| `paymenterror` | POST `/orders/checkout` | 502 | Downstream payment failure (503) |

Run `npm run scenarios -- fast`, `npm run scenario:checkout`, or `npm run scenario:all`.

### Manual checkout

```sh
curl -i http://127.0.0.1:3900/health
curl -i -X POST http://127.0.0.1:3900/orders/checkout \
  -H 'Content-Type: application/json' \
  -d '{"items":["sku-1"],"total":49.99,"mode":"slow"}'
```

Do **not** include card data, tokens, passwords, or actual customer information in test payloads.

## Important testing truths

- `orders` and `payments` are **separate HTTP processes**, unlike an in-process NestJS provider call. They test actual HTTP propagation when instrumentation is enabled.
- SQL, Redis, and messaging in this fixture are **explicitly mocked** spans with `lab.mock=true`. This tests trace ingestion/visualization but **does not** prove real driver auto-instrumentation or actual queue propagation.
- All endpoints are local-only and bounded in duration to reduce accidental load.
- Automatic instrumentation may not capture every Node.js operation; inspect collector results and assert known trace relationships before claiming a feature passes.
- The SDK is not silently substituted with OpenTelemetry in `xray` mode.
- Use the test matrix in `TEST_MATRIX.md` for manual and automation checks. Add additional applications in future (PostgreSQL/Redis Docker, Kafka, Java, Go, frontend) when those integrations exist.

## Typical visible trace

With the SDK/OTel enabled, the checkout route should provide a distributed trace resembling:

```text
POST /orders/checkout [lab-checkout]
  checkout.process
    order.insert.mock [lab.mock=true]
    HTTP GET /internal/inventory/sku-1 [lab-downstream]
      inventory.lookup.mock [lab.mock=true]
    HTTP POST /internal/payment [lab-downstream]
      payment.gateway.mock [lab.mock=true]
    order.complete.mock [lab.mock=true]
```

Exact span names and relationships depend on instrumentation version. This diagram illustrates intended correlation, not proof that tracing occurred.
