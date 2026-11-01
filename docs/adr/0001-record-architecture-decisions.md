# 1. Use monorepo structure with Rust core and TypeScript SDKs

Date: 2025-11-01

## Status

Accepted

## Context

Request X-Ray needs to be a cross-platform, high-performance observability tool that can instrument HTTP requests across multiple environments (backend services, frontend apps, edge functions). To achieve this, we need:
1. A core library that can be compiled to WebAssembly, native binaries, and used as a shared library
2. TypeScript/JavaScript SDKs for Node.js, browser, and edge runtimes that wrap the core
3. Easy dependency management and coordinated versioning across all components
4. Unified CI/CD for the entire project

## Decision

We will use a monorepo structure with:
- Rust for the core implementation (in `crates/`) that provides all the OTLP tracing, propagation, and collection logic
- TypeScript for all SDKs and higher-level packages (in `packages/`)
- pnpm for managing JavaScript dependencies and workspaces
- Cargo for managing Rust dependencies and workspaces
- Examples and test fixtures in dedicated directories (`examples/`, `tests/fixtures/`)

This structure allows us to maintain a single source of truth for all Request X-Ray components while enabling platform-specific SDKs that leverage the same core logic.

## Consequences

- All components are versioned together, ensuring compatibility between core and SDKs
- The Rust core can be compiled to multiple targets (WASM, native, library)
- CI workflows run once for the entire project, simplifying maintenance
- Contributors need to know both Rust and TypeScript to work on the full stack, but can focus on one language for specific components
