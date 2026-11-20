# Step 0 audit — 2026-11-01

**Result: Step 0 remains incomplete.** The 2026-11-19 re-audit below updates the original archive findings.

## Re-audit of `bd1a979` — 2026-11-19

| Item | Finding |
| --- | --- |
| 0.1 Name and ownership | The GitHub repository is recorded. npm scope and publisher rights, trademark review, and final naming sign-off are still pending. |
| 0.2 Governance | The license and governance policies are present. Issue and PR templates and verified private reporting contacts are missing. |
| 0.3 Toolchain | Rust 1.85.0, Node 20.18.0 in `.nvmrc`, pnpm 9.12.0 and lockfiles are tracked. The supported Node range and minimum Rust version policy still need to be reconciled with the CI toolchains. |
| 0.4 Scaffold | Both manifests and directory scaffolds are tracked. A fresh local clone installed with `corepack pnpm install --frozen-lockfile` and passed `corepack pnpm typecheck` on Node 20.20.2. The lab built but passed only four of five smoke tests. Checkout returned HTTP 502 instead of 201 on both runs. |
| 0.5 CI | Rust CI, Node.js CI, Security Audit and Secret Scanning reported success on this commit. The Node workflow does not run typecheck or the lab's smoke suite. The root lint and test commands skip packages without those scripts. |
| 0.6 ADRs | Seven ADRs are present. Their numbering differs from the architecture's ADR list. The architecture specifies platform npm packages for the native executable and still describes post-commit ACK as a preference pending a decision. |
| 0.7 Contracts | The draft OpenAPI file covers health only and lists port 4319. The architecture specifies 4731. A typed configuration schema and complete error contract remain pending. |
| 0.8 Documentation | Public install commands are labeled planned. The README still describes the repository as specification only despite the runnable test lab and protobuf crate. |

With the pinned Rust 1.85.0 toolchain, `cargo fmt --all --check`, `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`, and `cargo test --locked --workspace` passed. Run `corepack pnpm --filter request-xray-test-lab verify` to reproduce the lab check.

At the audited commit, Step 1.1 passed independent provenance and build verification, and `cargo test --locked -p xray-core` reported a missing package. Step 1.2 was subsequently implemented in `crates/xray-core`. See the [domain model verification](ARCHITECTURE.md#step-12-domain-model-verification).

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
