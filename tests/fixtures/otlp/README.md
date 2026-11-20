# OTLP exporter fixtures

These protobuf files are HTTP request bodies emitted by the official OpenTelemetry JavaScript OTLP/HTTP protobuf exporter 0.300.0 to a loopback listener. The generator uses OpenTelemetry SDK 3.0.0 with fixed IDs and times and synthetic data.

| Fixture | Coverage |
| --- | --- |
| `single.pb` | One root span with a one-nanosecond duration. |
| `distributed.pb` | Two service resources and instrumentation scopes, a parent with overlapping children, non-ASCII attributes and event text, a span link, an error status and an exporter-reported dropped attribute. |
| `large-attribute.pb` | One 64 KiB string attribute to exercise preservation and request byte limits. |

The Rust integration tests mutate the distributed capture in memory to produce invalid ID and timestamp variants. They also truncate a captured body to test malformed protobuf. The files here are the original captures.

From the repository root, using Node 20.20.2 and pnpm 9.12.0:

```sh
corepack pnpm install --frozen-lockfile
node test/scripts/generate-otlp-fixtures.cjs
cargo test --offline --locked -p xray-otlp --test exporter_fixtures
```

Pass an existing directory as the generator's optional argument to compare regenerated files without rewriting the tracked fixtures.
