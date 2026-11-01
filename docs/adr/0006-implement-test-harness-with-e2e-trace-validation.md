# 6. Implement test harness with end-to-end trace validation

Date: 2025-11-01

## Status

Accepted

## Context

Request X-Ray needs reliable testing that verifies traces are correctly generated, propagated, and exported across the entire system. We need a test harness that:
1. Can spin up temporary OTLP collectors to capture exported traces
2. Validates that trace structure, attributes, and context propagation are correct
3. Works with both Rust and TypeScript SDKs in a unified way
4. Supports unit, integration, and end-to-end testing scenarios
5. Integrates with our existing CI workflows to run on every PR

## Decision

We will build a dedicated test harness that includes:
1. A Rust library (`request-xray-test-utils`) that spins up in-memory OTLP collectors for Rust crate testing
2. A TypeScript test utility package (`@request-xray/test-utils`) that does the same for Node.js/browser tests
3. A NestJS test application (in the `test/` directory) to instrument and validate end-to-end trace flow
4. Trace validation logic that checks W3C compliance, attribute correctness, and span relationships
5. CI integration that runs the full test suite on every push and PR

The NestJS test application will serve as the primary way to validate that our packages work correctly in a real-world server environment.

## Consequences

- We have a single, consistent way to test all Request X-Ray components
- E2E tests catch integration issues that unit tests might miss
- The test harness is reusable across all SDKs and crates
- We can validate OTLP export correctness without relying on external services
- Additional maintenance cost to keep the test harness updated with new features
