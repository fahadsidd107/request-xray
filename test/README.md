# Request X-Ray Integration Test Lab

A **private, disposable NestJS application** to place at `<request-xray-repo>/test/`. It is an integration fixture for a Rust-powered Request X-Ray collector, npm SDK, and dashboard. It contains two independently running NestJS HTTP servers, realistic test scenarios, named manual spans, an optional OpenTelemetry exporter, and Node's built-in smoke tests.

The lab runs within the pnpm workspace and uses synthetic data. Node 22 and 24 are supported in Request X-Ray mode.

## Start the lab

From the repository root:

```sh
corepack pnpm install --frozen-lockfile
corepack pnpm --filter @request-xray/node build
corepack pnpm --filter request-xray-test-lab build
cargo run --locked -p xray-collector
```

In a separate terminal run `corepack pnpm --filter request-xray-test-lab start:xray`, then `corepack pnpm --filter request-xray-test-lab scenario:all`. The checkout service listens on `127.0.0.1:3900` and downstream on `127.0.0.1:3901`. The SDK starts before NestJS imports, then sends spans to `127.0.0.1:4318/v1/traces`. Override the endpoint with `XRAY_COLLECTOR_URL`. Inspect committed traces through the [query API](../docs/QUERY_API.md).

Use `start:otel` to test the official OpenTelemetry exporter separately, or `start` to run without tracing. `corepack pnpm --filter request-xray-test-lab verify` covers typechecking, build, and ordinary HTTP smoke tests. The [SDK integration test](../docs/NODE_SDK.md#verification) exercises NestJS with real persistence and trace assertions.

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
