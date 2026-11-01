# 5. Choose GitHub Actions as the sole CI/CD platform

Date: 2025-11-01

## Status

Accepted

## Context

Request X-Ray needs a CI/CD platform that can:
1. Build both Rust and TypeScript code in the same pipeline
2. Support cross-platform testing (Linux, macOS, Windows)
3. Integrate natively with GitHub's release and package publishing features
4. Have sufficient free minutes for open-source development
5. Support caching for Rust and Node.js dependencies to speed up builds
6. Integrate with security scanning tools (secret scanning, dependency audits)

## Decision

We will use GitHub Actions as the only CI/CD platform for Request X-Ray. We will:
1. Implement all build, test, and release workflows using GitHub Actions
2. Leverage GitHub's native caching for cargo and pnpm dependencies
3. Use GitHub's trusted publishing to publish npm packages to the npm registry
4. Use GitHub Releases to publish native binaries and WASM packages
5. Avoid integrating with third-party CI/CD platforms (CircleCI, GitLab CI, etc.) unless absolutely necessary

## Consequences

- Simple, single-platform maintenance for all CI/CD workflows
- Native integration with all GitHub features (PR checks, releases, packages)
- Sufficient free CI minutes for an open-source project
- Easy for contributors to understand and modify CI workflows
- Lock-in to GitHub's ecosystem, but this is a low-risk choice for most open-source projects
