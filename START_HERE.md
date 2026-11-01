# Request X-Ray — corrected repository handoff

This is the **entire source tree**, not a patch. It combines the uploaded `request-xray.zip` with the Step 0 audit corrections. Existing source, docs, ADRs and test lab are preserved.

## Important

- The archive intentionally excludes `.git/`, `target/`, `node_modules/`, macOS metadata and other generated output. Keep your existing `.git` history when copying these files into an existing checkout.
- npm/scope ownership, trademark/domain checks, security/contact setup and CI success **are not confirmed**; Step 0 has not passed its exit gate.
- `pnpm-lock.yaml` was missing in the uploaded project. It could not be generated here because npm registry access is unavailable. Generate and commit it locally **before enabling/running the frozen-lockfile GitHub CI workflow**.
- Full Rust/TypeScript builds and package installation could not be verified in this environment.

## Use as a new folder

Extract the ZIP. Open the `request-xray/` directory.

## Use with your existing Git checkout

Back up or commit your current changes. Copy the extracted `request-xray/` **contents** into your existing repository, keeping the existing `.git/` folder. Review `git diff` before committing.

## Required local verification

```bash
corepack enable
corepack pnpm install
# Commit the newly generated pnpm-lock.yaml
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
corepack pnpm typecheck
corepack pnpm test
git diff --check
```

Then inspect GitHub Actions results and complete `docs/STEP0_AUDIT.md` and `docs/NAME_VERIFICATION.md`. Only after Step 0 is verified begin Step 1.1.
