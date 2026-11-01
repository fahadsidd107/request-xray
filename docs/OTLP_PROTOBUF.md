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
4. `xray-otlp` exposes generated wire types. Domain validation and conversion are deliberately reserved for Steps 1.2–1.4.

## Updating the protocol later

Do **not** switch to `main` or replace schemas by hand. In a separate reviewed PR:

1. Review upstream release notes, protobuf compatibility, and license.
2. Change pinned tag and **full commit hash** in both scripts and this document.
3. Remove only `schemas/vendor/opentelemetry-proto` and rerun `bash scripts/otlp-sync.sh`.
4. Review vendored `.proto` diffs and commit the upstream license + source changes.
5. Run the generator tests, update `Cargo.lock`, run the full integration suite and CI on macOS/Linux/Windows.
6. Record breaking or newly added fields; update canonical conversions and fixtures in their respective roadmap steps.

## Verification limitations

The sync checks the Git ref commit before copying. The lightweight offline verify script checks paths and metadata, **not cryptographic per-file hashes**; the pinned committed file contents plus code review are the build inputs. For strongest provenance, reviewers should independently compare the vendored tree against the pinned commit or record SHA-256 digests. No network-backed compilation has been run in this generated patch environment.
