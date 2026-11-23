# Node instrumentation SDK

`@request-xray/node` is available in this pnpm workspace. Run the collector from source and build the SDK before using the examples.

```sh
corepack pnpm install --frozen-lockfile
corepack pnpm --filter @request-xray/node build
cargo run --locked -p xray-collector
```

The SDK supports Node.js 22 and 24, with CommonJS and ESM entry points. Automatic capture covers Node HTTP and HTTPS, Express routes, and Undici-backed outbound requests including built-in `fetch`. The NestJS fixture uses the Express adapter. [Node.js release schedule](https://nodejs.org/en/about/previous-releases)

## Preload before importing the application

Run the examples in two separate terminals from the repository root. The ESM preload registers the OpenTelemetry loader hook before the entry point imports Express or HTTP. The CommonJS preload starts the SDK synchronously.

```sh
corepack pnpm --filter request-xray-express-example exec env OTEL_SERVICE_NAME=orders-downstream \
  node --require @request-xray/node/register downstream.cjs
corepack pnpm --filter request-xray-express-example exec env OTEL_SERVICE_NAME=orders-api \
  node --import @request-xray/node/register app.mjs
```

```sh
curl http://127.0.0.1:3900/orders/42
curl 'http://127.0.0.1:4731/api/v1/traces?service=orders-api'
```

Expect one trace with both services, inbound Express routes, a manual `orders.lookup` span and child requests from both `fetch` and `node:http`. Use the returned trace ID with `/api/v1/traces/{id}/spans` to inspect the persisted relationship.

For programmatic startup, call `startXRay(options)` from a preload. In a custom ESM preload, register the hook using `register('@opentelemetry/instrumentation/hook.mjs', import.meta.url)` before loading the SDK and app. The packaged ESM preload already does this. CommonJS detects Express loaded too early and raises a startup error. Initialize exactly once per process.

## Configuration and lifecycle

| Option | Default |
| --- | --- |
| `serviceName` | `OTEL_SERVICE_NAME` or `request-xray-app`. |
| `serviceVersion`, `environment` | Unset. These set `service.version` and `deployment.environment.name`. |
| `collectorEndpoint` | `REQUEST_XRAY_COLLECTOR_ENDPOINT`, then `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT`, then `http://127.0.0.1:4318/v1/traces`. |
| `capture.http`, `capture.express`, `capture.fetch` | Enabled. Each can be disabled independently in a custom preload. |
| `sampling` | Parent-based sampling with a root probability of 1. Set `{ mode: 'parentbased_traceidratio', probability: 0.25 }` to sample a quarter of new roots. |
| `maxQueueSize`, `maxExportBatchSize`, `scheduledDelayMs` | 256 spans, up to 32 spans per batch, 500 ms. |
| `exportTimeoutMs`, `shutdownTimeoutMs` | 2000 ms per export and 5000 ms for a flush or shutdown call. |

The exporter sends OTLP/HTTP protobuf. It has one concurrent export and drops excess spans when the bounded queue fills. Collector errors do not fail application requests. In standalone mode, the propagator uses W3C `traceparent` and `tracestate`. Baggage propagation is disabled. The SDK does not automatically detect host or process resource attributes.

Use `getXRay()` to retrieve the preload handle. Stop accepting requests before awaiting `handle.shutdown()`. Shutdown is idempotent and disables automatic instrumentation. Both `flush()` and `shutdown()` return a boolean indicating whether the SDK operation completed within its deadline. A successful flush is the SDK's lifecycle result. Collector persistence is verified separately through the query API. The SDK leaves signal handling to the application.

## Manual instrumentation

```ts
import { addEvent, getActiveSpan, withSpan } from '@request-xray/node';

const result = await withSpan('orders.lookup', async () => {
  const order = await findOrder();
  addEvent('orders.lookup.complete', { 'order.found': Boolean(order) });
  return order;
});
```

`withSpan` ends its span on success or failure, marks thrown or rejected operations as errors, and preserves the original error. It accepts a synchronous or asynchronous callback and returns a promise. `getActiveSpan()` is optional and `addEvent` does nothing without an active span.

Automatic instrumentation does not capture bodies or raw headers. Before export the SDK strips known credential fields, headers, cookies, query strings, URL userinfo, exception messages and stacks, and free-form status messages. It applies the same rules to manually recorded attributes, events and links in standalone mode. Keep arbitrary sensitive values out of custom names and attribute values. Classify manual metadata before recording it. Collector redaction remains enabled.

## Existing OpenTelemetry provider

The default startup refuses to install a second global provider. To add the Request X-Ray destination, include `createXRaySpanProcessor({ collectorEndpoint })` in the existing provider's span processors at construction. Keep the application's instrumentation, sampling, propagation and lifecycle configuration. The Request X-Ray processor sanitizes its own exported snapshot; any other existing exporter uses its own policy.

To use only the manual helpers with an existing global provider, call `startXRay({ provider: 'use-existing' })`. This mode installs no instrumentation or exporter. Its handle exposes `ownsProvider: false` and leaves flush and shutdown to the provider owner.

## Verification

```sh
corepack pnpm --filter @request-xray/node build
corepack pnpm --filter request-xray-test-lab build
corepack pnpm --filter @request-xray/node test
corepack pnpm --filter @request-xray/node test:e2e
```

Build the native collector with `cargo build --locked -p xray-collector` before the end-to-end test. CI runs this test on Node 22 and 24. The test sends real SDK exports to a temporary SQLite collector and checks CommonJS and ESM preloads, HTTP and fetch spans, Express route templates, W3C propagation, raw protobuf privacy before collector redaction, NestJS checkout, and application behavior during a collector outage.

Verified locally on Node 22.23.4 and 24.22.2. Both passed four SDK unit tests, the collector integration test, five NestJS HTTP smoke tests, and workspace typechecking. The collector integration also confirmed that a processor attached to an existing OpenTelemetry provider exported a persisted span.
