# Step 1.1 — Official OTLP protobuf provenance and generation

## Authority

- Upstream: https://github.com/open-telemetry/opentelemetry-proto
- Release tag: `v1.11.1` (released September 2026)
- Pinned **full commit**: `b3f75588eb23c5fca62264edd05d382de49beb1a`
- Upstream license: Apache-2.0; a copy travels with the vendored sources under `schemas/vendor/opentelemetry-proto/LICENSE`.
- Scope for v0.1: `common/v1`, `resource/v1`, `trace/v1`, `collector/trace/v1`. Metrics, logs and profiles are intentionally out of scope for the trace-only receiver.

## How it works

Run from repository root:

```bash
bash scripts/otlp-sync.sh
bash scripts/otlp-verify.sh
cargo test -p xray-otlp
cargo clippy -p xray-otlp --all-targets -- -D warnings
cargo fmt --all --check
```

1. `otlp-sync.sh` fetches the upstream release tag and **checks its full commit ID** before copying `.proto` files and Apache license.
2. It vendors the exact input files in git, so subsequent builds do **not** need a network connection.
3. The Rust `build.rs` uses `prost-build = 0.13.5` and `protoc-bin-vendored = 3.1.0` (versions must also be locked in `Cargo.lock`) to generate Rust code into Cargo `OUT_DIR` during builds. Do not check in generated Rust files.
4. `xray-otlp` exposes generated wire types and the trace decoder described below.

## Trace decoding and conversion

`xray_otlp::decode_export(&[u8])` decodes an `ExportTraceServiceRequest` and returns the ordered resource, scope and span hierarchy in `xray-core`. It preserves schema URLs, optional message presence, attributes and dropped counts, trace state, flags, events, links and unknown kind and status codes. Profile-only dictionary references have no trace semantics. The mapper ignores key reference indexes and represents string reference values as empty.

Use `decode_export_report(payload, max_payload_bytes)` to apply a byte limit before decoding and obtain a batch with its rejected-span tally. The limit is inclusive and applies to the uncompressed protobuf bytes. Each span with an invalid ID, timestamp or interval counts once. Valid spans remain in the output even if a rejected span was their parent. Resource and scope metadata and ordering are preserved even when a group becomes empty. An empty request returns an empty batch with a tally of zero. A request-level error has no span tally.

`decode_export(payload)` remains the strict entry point for callers that apply their own byte limit. It returns the first invalid span with zero-based resource, scope and span indexes and a field-specific cause. `convert_span` validates one decoded span.

| Category | Condition and handling |
| --- | --- |
| `invalid_payload` | Malformed OTLP protobuf. Reject the request. |
| `resource_limit` | The payload exceeds the configured byte limit. Reject before protobuf decoding. Compression and finer-grained ingestion limits are Step 2 work. |
| `invalid_span` | An ID, timestamp or interval failed validation. Strict conversion returns an error. Report mode omits the span and increments the rejected-span tally. |
| `storage_unavailable` | Reserved for ingestion when accepted spans cannot be committed. The decoder has no storage dependency. HTTP response, retry and acknowledgment handling belong to Steps 2–3. |

Expose the stable category from `DecodeError::code()` for decoder diagnostics. Report mode returns decoded data and a rejection count. The eventual collector must decide the OTLP response after storage and must account for conversion rejections before acknowledging an export. Decoder error messages contain structural positions and field details.

## Exporter fixtures and verification

The [fixture directory](../tests/fixtures/otlp/README.md) contains byte-for-byte HTTP protobuf bodies from the official OpenTelemetry JavaScript exporter. It covers a single span, a nested trace across two services with overlapping work, and a 64 KiB attribute. The Rust fixture tests derive malformed-ID, invalid-time and truncated variants from the captured distributed export.

Run `cargo test --offline --locked -p xray-core -p xray-otlp`. Expect 17 passing tests across both crates, including four exporter fixture tests and two property tests. The properties exercise arbitrary bounded protobuf payloads and verify that valid IDs, ordered duplicate attributes, dropped counts, scope schema URLs and status codes survive conversion.

## Updating the protocol later

Do **not** switch to `main` or replace schemas by hand. In a separate reviewed PR:

1. Review upstream release notes, protobuf compatibility, and license.
2. Change pinned tag and **full commit hash** in both scripts and this document.
3. Remove the old vendor directory and `schemas/otlp-sha256.txt` in the update branch, then rerun `bash scripts/otlp-sync.sh`.
4. Review vendored `.proto` diffs and commit the upstream license + source changes.
5. Run the generator tests, update `Cargo.lock`, run the full integration suite and CI on macOS/Linux/Windows.
6. Record breaking or newly added fields; update canonical conversions and fixtures in their respective roadmap steps.

## Verification

On 2026-11-19, a fresh sync in a temporary directory resolved the tag to the pinned commit. The new vendor tree and SHA-256 manifest matched the tracked copies byte for byte. Run `bash scripts/otlp-verify.sh` to check the pin and manifest offline. Expect `Verified pinned OTLP revision and SHA-256 checksums.` Appending a comment to the temporary copy of `trace.proto` produced a checksum failure and a nonzero exit.

With Rust 1.85.0, workspace formatting, Clippy with warnings denied, and the locked Rust tests passed. The `xray-otlp` test encodes and decodes an export request using the generated wire types.
