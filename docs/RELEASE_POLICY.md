# Release and Versioning Policy

**Status:** Proposed policy. No release has been published or validated yet.

## Versioning

Use Semantic Versioning for user-facing npm packages and Rust libraries. Before 1.0.0, minor releases can introduce breaking changes, but they must be clearly documented. Publicly released package versions must be immutable; publish a new version rather than rewriting an existing tarball.

- `0.1.0`: first working end-to-end alpha candidate, only after acceptance criteria pass.
- `0.x`: iterative development; announce incompatible SDK/API changes in release notes.
- `1.0.0`: stable compatibility and documented production support policy.

Git tags use `vX.Y.Z` for coordinated project releases. If separate package versions later become necessary, define an ADR and migration approach first.

## Branches and changes

- Default branch: `main` (proposed).
- Work in feature/fix branches with reviewed PRs.
- Keep a human-readable `CHANGELOG.md` beginning with the first release.
- Highlight breaking changes, migration guidance, fixes and security updates.

## Minimum release gates

1. Ownership and npm name availability verified live; repository and scope permissions confirmed.
2. Version and public APIs finalized, docs/examples checked against the code.
3. `cargo fmt`, `cargo clippy`, Rust tests and workspace checks green.
4. JS/TS lint, typecheck, unit and integration tests green.
5. Real OTLP → Rust collector → SQLite → query API → dashboard smoke test green.
6. Supported platform install/build tests pass (at least declared OS/architectures).
7. License notices, dependency policy and security checks pass; no secrets in artifacts.
8. Dry-run packaging and unpacked smoke test confirms included assets and executable permissions.
9. Release notes, upgrade/migration guidance and known limitations are ready.
10. Maintainer explicitly authorizes publish; never auto-publish from an unreviewed PR.

## Artifact and npm publication

Expected flow once implementation exists:

```bash
# Illustrative only: exact scripts and workspaces established later
cargo test --workspace
pnpm -r test
npm pack --dry-run
# Inspect resulting tarball, run smoke install, then publish on approval
```

Prefer npm trusted publishing with OIDC and provenance where supported. For native Rust binaries, build per platform, checksum release artifacts, verify optional-dependency installation and confirm graceful errors for unsupported targets. Publish from an authenticated, protected release workflow only.

## Rollback and incidents

npm package versions cannot be overwritten. If a release is faulty, publish a patched version and document mitigation. Use npm deprecation for affected versions when appropriate. Coordinate security disclosures privately and avoid deleting published versions as a routine rollback method.

## Support and deprecation

Before 1.0, state supported platforms and limitations in each release. Set a formal supported-version window and end-of-life process before claiming production readiness. Deprecate functionality with a documented replacement path and compatibility period where practical.

## Release ownership

Only verified package maintainers with explicit authorization may publish. The owner and security contacts remain unresolved in `docs/MAINTAINERS.md`.
