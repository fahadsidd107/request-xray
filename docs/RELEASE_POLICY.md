# Release and Versioning Policy

**Status:** Preparing the 0.1.0 release candidate. Registry publication is pending.

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

## Release checklist

- Confirm the version matches the native CLI, launcher, every platform package and Node SDK. Review the changelog and the README commands.
- Run the [packaging workflow](../.github/workflows/npm-cli.yml) on the intended commit. Require both Node 22 and 24 to pass on all five supported targets. Review the tarball file lists, license files, supported targets and absence of lifecycle hooks.
- Save the exact five platform tarballs, launcher and SDK from green jobs. Select one launcher and one SDK tarball for the release, and retain the checksum files that correspond to those exact bytes. Keep the SPDX source dependency inventory with the evidence. It covers the source dependency graphs, including build-time dependencies.
- Check Rust and npm audits, secret scanning, collector security tests and the SDK credential-capture regression. Resolve findings before release. Keep the actual clean-install logs for macOS ARM64 and Linux x64.
- Confirm registry ownership and the first-publication path for all seven packages. Configure npm trusted publishing after package settings are available. Use a public repository and the exact approved workflow identity for provenance. See [npm provenance](https://docs.npmjs.com/generating-provenance-statements/) and [npm trusted publishing](https://docs.npmjs.com/trusted-publishers/).

## Artifact and npm publication

Place the selected tarballs in one directory and run `node scripts/publish-npm-release.cjs <artifact-directory>` to inspect them and check registry state. The preflight is read-only. It verifies package metadata, licenses, platform constraints, the native binary hashes and the exact optional dependency versions. For every version already present it requires the published SHA-512 integrity to match the reviewed tarball.

After explicit owner authorization and green gates, create the annotated `vX.Y.Z` tag at the reviewed commit. Create a draft GitHub release and attach the reviewed artifacts, checksum files, SPDX SBOM and release notes. Run the publication command in the owner-approved GitHub-hosted workflow with `contents: read` and `id-token: write` permissions, a protected release environment, a supported npm CLI, and the required npm registry credentials for any first publication. Supply the approved tag as the second argument to the script. The tagged checkout must be clean.

The script publishes all native platform packages first, then the matching Node SDK, then the CLI launcher last. If publication stops partway, retain the exact tarballs and rerun with the same tag and files. Packages already present with matching integrity are skipped. A mismatch stops the release and requires maintainer review. Never delete or replace an existing package version to make a retry pass.

After publishing, use `npm view <name>@<version> name version dist.integrity --json` for all seven names and verify provenance on npm. Install just `request-xray@<version>` and `@request-xray/node@<version>` into a clean project from the registry. Confirm npm selected the native dependency automatically, run `request-xray --version`, then follow the README demo and doctor commands on macOS ARM64 and Linux x64. Promote the GitHub draft after verification passes.

## Rollback and incidents

npm package versions cannot be overwritten. If a release is faulty, publish a patched version and document mitigation. Use npm deprecation for affected versions when appropriate. Coordinate security disclosures privately and avoid deleting published versions as a routine rollback method.

## Support and deprecation

Before 1.0, state supported platforms and limitations in each release. Set a formal supported-version window and end-of-life process before claiming production readiness. Deprecate functionality with a documented replacement path and compatibility period where practical.

## Release ownership

Only verified package maintainers with explicit authorization may publish. The owner and security contacts remain unresolved in `docs/MAINTAINERS.md`.

## Release rehearsal on 2026-11-25

| Clean project | Runtime | Installed output |
| --- | --- | --- |
| macOS ARM64 | Node v22.23.4 | `request-xray 0.1.0`, query and OTLP readiness `ok`, `11111111111111111111111111111111  60 ns  3 spans  error`, `found 0 vulnerabilities` |
| Linux x64 (Debian Bookworm container) | Node v24.22.3 | `request-xray 0.1.0`, query and OTLP readiness `ok`, `11111111111111111111111111111111  60 ns  3 spans  error`, `found 0 vulnerabilities` |

The clean installs used local CLI, platform and SDK tarballs and fetched SDK dependencies from the public npm registry. Both SDK module entry points loaded. The Linux rehearsal used the dashboard-enabled native CLI from [the earlier five-target run](https://github.com/fahadsidd107/request-xray/actions/runs/45649415386). Full console output is in the local workspace under `target/step8-macos/fresh-install.log` and `target/step8-ci-linux-input/fresh-install.log`. The local SPDX 2.3 lockfile inventory is under `target/step8-sbom/request-xray-source.spdx.json`.

Collector tests passed for loopback binding, redaction before persistence, body and structure limits, admission capacity and timeouts. The Node SDK end-to-end test verified credential, body and query test values were absent from the exported OTLP payload. The release audit remains open for [libsqlite3-sys](https://rustsec.org/advisories/RUSTSEC-2026-0459.html) and [RSA](https://rustsec.org/advisories/RUSTSEC-2023-0071.html) on the lockfile. RSA is absent from the active macOS CLI dependency tree. The `url` dependency was upgraded to 2.5.9 and the CLI build check and collector tests passed with the updated lockfile.
