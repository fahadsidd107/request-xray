# Request X-Ray Versioning Standards

We follow strict [Semantic Versioning 2.0.0](https://semver.org/) for all Request X-Ray packages and crates. This document outlines specific conventions for our monorepo.

## Core Versioning Rules

For all crates (`crates/`) and npm packages (`packages/`):
- **MAJOR version** (X.0.0): Breaking changes that require users to update their code
- **MINOR version** (0.X.0): New functionality added in a backward-compatible manner
- **PATCH version** (0.0.X): Backward-compatible bug fixes and documentation updates

## Monorepo-Specific Conventions

1. **Synchronized versions**: All packages in the monorepo share the same version number for simplicity
2. **Pre-1.0.0 stability**: Before 1.0.0, even minor versions (0.X.0) may contain breaking changes - we will explicitly call out breaking changes in release notes
3. **API versioning**: Our OpenAPI schemas use a separate `/v1/` path versioning scheme to avoid coupling API changes to package versions
4. **Git tags**: All releases are tagged with `v{version}` (e.g., `v0.1.0`)
5. **Pre-releases**: Use SemVer pre-release tags for development versions:
   - `0.1.0-alpha.1` for early alpha releases
   - `0.1.0-beta.1` for beta releases
   - `0.1.0-rc.1` for release candidates

## Dependency Versioning

- When updating dependencies, we follow the same SemVer rules for compatibility
- All workspace dependencies are pinned to specific versions in root Cargo.toml and package.json
- We run `pnpm dedupe` and `cargo update` regularly to keep dependencies updated while maintaining compatibility
