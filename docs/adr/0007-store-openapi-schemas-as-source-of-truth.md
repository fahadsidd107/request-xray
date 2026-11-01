# 7. Store OpenAPI schemas as source-of-truth for API contracts

Date: 2025-11-01

## Status

Accepted

## Context

Request X-Ray exposes APIs for configuring collectors, querying stored traces, and managing instrumentation. We need a way to define these APIs that:
1. Ensures consistency between documentation, client SDKs, and server implementations
2. Supports automatic generation of client libraries and server stubs
3. Enables contract testing between services
4. Provides interactive documentation that's always up-to-date
5. Works with both our Rust and TypeScript codebases

## Decision

We will use OpenAPI 3.x as the source-of-truth for all Request X-Ray API contracts. We will:
1. Store all OpenAPI schema files in a dedicated `/schemas` directory at the repository root
2. Use `openapi-generator` to generate TypeScript client SDKs and Rust server stubs from the schemas
3. Use `swagger-ui` to serve interactive API documentation from our collectors
4. Implement contract testing in CI to verify that server implementations match the OpenAPI schemas
5. Never modify generated code directly - all changes must be made to the source OpenAPI schemas first

## Consequences

- All API changes are explicitly documented in the OpenAPI schemas before implementation
- Client SDKs are always in sync with server API versions
- Interactive documentation is always up-to-date with the latest API changes
- Contract testing catches breaking changes early in the development process
- Additional tooling required to maintain and generate code from OpenAPI schemas
