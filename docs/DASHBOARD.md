# Local trace dashboard

The dashboard runs in the browser on the same loopback origin as the query API. It reads committed traces through the generated TypeScript client. Demo mode loads a labeled synthetic trace into an isolated database.

From the repository root:

```sh
corepack pnpm install --frozen-lockfile
corepack pnpm --filter @request-xray/query-client build
corepack pnpm --filter @request-xray/dashboard build
XRAY_REQUIRE_DASHBOARD=1 cargo run --locked -p xray-cli -- dev --no-open
```

Open the printed URL. Start the [Express example](NODE_SDK.md#preload-before-importing-the-application) to send real spans. Substitute `demo` for `dev` to explore built-in example data. npm platform builds embed the compiled app in the native binary and need no separate web server or browser assets on disk. A source-only Rust build with no dashboard bundle retains the compact CLI trace view.

## Investigate a trace

The overview shows query health, committed trace totals, error traces, and the first 20 observed services. Use the exact service filter to find additional services. An empty database shows setup information. The trace table uses the API's stable keyset pages and supports trace ID, service, route, status, and timing filters. Refresh to read newly committed spans and summaries.

Open a trace to compare span offsets and durations in the waterfall. Expand parent rows to inspect their children. Use the zoom and pan controls to move through overlapping spans. Select a span for its exact timestamp and duration, kind and status, resource, attributes, events, and links. The waterfall renders only visible rows plus a small surrounding buffer. Loading stops at 10,000 spans or 64 MiB of aggregate page payload and displays a notice if either limit is reached.

Missing parents can come from an unobserved service, sampling, or late arrival. Reported dropped attributes, events, and links describe exporter-side losses. Sampling coverage is unknown. A child interval outside its parent's interval may reflect clock skew or asynchronous work. The waterfall retains recorded offsets and overlap.

## Verify the installed dashboard

Build and stage a CLI with the dashboard included, then run the Playwright suite:

```sh
corepack pnpm --filter @request-xray/node build
XRAY_REQUIRE_DASHBOARD=1 cargo build --locked --release -p xray-cli
node scripts/stage-npm-cli.cjs target/release/request-xray-native npm-cli-stage
XRAY_REQUIRE_DASHBOARD=1 node scripts/smoke-npm-cli.cjs npm-cli-stage
cargo run --locked -p xray-otlp --example dashboard_fixtures -- target/dashboard-playwright/fixtures
corepack pnpm exec playwright install chromium
corepack pnpm test:dashboard
```

Use `XRAY_NPM_STAGE` if tarballs were staged elsewhere. The suite installs the npm tarballs in a clean project. It checks an empty database, spans from the real instrumented Express example, exact identifiers and nanoseconds, a missing parent, and a labeled demo error. Separate synthetic load fixtures benchmark the 1,000- and 10,000-span waterfall through real OTLP ingestion and the paged SQLite API. The test records time from trace navigation to all pages loaded and the number of mounted waterfall rows.

## Verification results

On 2026-11-24, the macOS ARM64 release tarballs passed the offline install smoke and four Chromium Playwright tests. The browser verified both Express services, the exact persisted span ID and nanoseconds, trace filtering and keyset navigation, keyboard operated zoom and pan, demo labeling, a collector error state, and late-parent reconciliation. The suite also checked that exporter drop counts and possible cross-service clock skew remain visible. The visual review confirmed desktop, tablet and mobile layouts, including the compact height for short traces.

One local release run measured 82 ms to load a 1,000-span trace with 24 mounted rows and 289 ms for a 10,000-span trace with 26 mounted rows. These are measurements from navigation through the local paged SQLite API to a rendered waterfall, excluding fixture ingestion. The benchmark uses small span payloads. Browser, hardware, and attribute payload size affect timings.
