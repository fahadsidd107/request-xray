# Step 0 audit — 2026-11-01

**Result: NOT COMPLETE. Do not begin Step 1.1 until the exit criteria are proven.**

## Findings from supplied archive

- 0.1: `docs/NAME_VERIFICATION.md` explicitly says npm, GitHub ownership, trademarks and domain are unconfirmed. Owner must verify current authority and record evidence. Domain can be deferred.
- 0.2: governance files present; GitHub issue and PR templates absent in archive. Private security reporting, conduct contact and maintainer sign-off still unresolved. Templates are included in this patch.
- 0.3: Rust toolchain is pinned and root npm manifest exists, but package manager lockfile was ignored; pnpm workspace manifest absent; no reproducible Node CI setup.
- 0.4: Rust Cargo scaffold and directories exist, but pnpm workspace missing and only placeholder Rust crate exists. CI smoke proof not supplied.
- 0.5: Rust CI uses stable rather than pinned toolchain; Node jobs cache pnpm before pnpm setup, and expected root scripts were absent. Audit cannot certify CI as green; update workflow, generate/commit lockfile, and run jobs on GitHub.
- 0.6: 7 ADR files exist but ADR-0001/0006 are dated 2025, and ACK semantics and native binary distribution need clear explicit decisions and review against the normative architecture.
- 0.7: existing OpenAPI file described an unimplemented collector at 4317 and a fictional production domain; replaced with a clearly planned query API at 4319. Add complete API error envelope/config contract before marking complete.
- 0.8: README and docs emphasize pre-release, but the roadmap incorrectly claimed completed tasks and README references old paths; correct links and status after owner review.

## Owner / local verification sequence

1. Verify the names and official ownership; fill `docs/NAME_VERIFICATION.md`, configure confidential reporting and validate GitHub account handle.
2. Review the patch; merge into repository root, preserving existing work.
3. Install pinned Rust toolchain and Node/Corepack; generate `pnpm-lock.yaml` using `corepack pnpm install`, then **commit it**.
4. Run `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `corepack pnpm install --frozen-lockfile`, `corepack pnpm typecheck`, `corepack pnpm test`.
5. Verify CI logs and update ADRs and API/config contracts. Record exact commit and test evidence before checking 0.1–0.8.

Environment limitation: full Rust/pnpm builds were not run here because Rust/pnpm tooling was not available. The ZIP was inspected statically; do not interpret this audit as an executable validation.
