# Contributing to Request X-Ray

Thank you for contributing. Request X-Ray is **pre-implementation**; commands below are planned until the workspace exists.

## First steps

1. Read [`README.md`](README.md), [`ARCHITECTURE.md`](ARCHITECTURE.md), and [`ROADMAP.md`](ROADMAP.md).
2. Check existing issues and milestones before starting substantial work.
3. For an API/protocol/storage/design change, open a design issue or ADR proposal before implementation.
4. Keep changes focused; reference a roadmap item and include proof of testing.

## Development principles

- Rust for collection, storage, analysis, and CLI; TypeScript/OpenTelemetry for Node instrumentation.
- OTLP and W3C trace context are compatibility contracts; do not invent wire schemas.
- Do not record credentials, request bodies, payment data, raw SQL parameters, or personal data by default.
- Handle errors explicitly, bound memory and queues, and do not interrupt host applications when telemetry fails.
- Never claim causal certainty from spans alone. Separate facts from hypotheses.
- Do not introduce network access to AI providers without explicit opt-in and redaction.

## Repository layout (target)

- `crates/`: Rust workspaces and integration primitives.
- `packages/`: npm SDK, CLI wrapper, dashboard, types.
- `docs/`: ADRs, maintainership and release policies.
- `test/`: integration laboratory and scenario fixtures.

## Tooling

The supported Rust, Node.js, and package-manager versions will be pinned in Roadmap Step 0.3. Avoid assuming a version that has not been adopted. After bootstrap, expected checks include:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
pnpm -r lint
pnpm -r typecheck
pnpm -r test
```

These commands **will not all run until their manifests/scripts are created**. For now, report which checks you could actually run.

## Pull requests

- Use the [PR template](.github/PULL_REQUEST_TEMPLATE.md).
- Reference an issue or roadmap item; document behavior before/after.
- Include appropriate unit/integration tests, fixtures and migration notes.
- Update docs and API examples when contracts change.
- Explain data-handling, telemetry overhead, compatibility, and rollback effects.
- Include a `cargo fmt`/Clippy/test and package-check summary, including skipped checks.
- Do not commit secrets, credentials, production telemetry, compiled binaries, or vendored dependencies without review.
- Maintainers review and approve changes. Contributors must not self-merge unless explicitly authorized by policy.

## Commit and change log conventions

Prefer descriptive commits such as `feat(collector): accept OTLP trace exports` or `fix(storage): reject duplicate conflicting spans`. Breaking changes must be labeled explicitly. Release notes follow `docs/RELEASE_POLICY.md`.

## DCO / CLA

Neither a CLA nor DCO sign-off is required initially. If a future policy adds one, it must be discussed and documented before enforcement; do not retroactively claim contributions were signed.

## Licensing

By contributing, you agree your contributions will be distributed under the repository's [Apache License 2.0](LICENSE), and affirm you have the right to submit them. Preserve required third-party attribution and licenses.

## Security

Do not publicly disclose vulnerabilities in issues or PRs. Follow [`SECURITY.md`](SECURITY.md).
