# 3. Use W3C Trace Context for distributed trace propagation

Date: 2025-11-01

## Status

Accepted

## Context

Request X-Ray needs to propagate trace context across distributed services, supporting multiple programming languages and runtimes. We need a propagation standard that:
1. Is universally adopted across the industry
2. Works with all major cloud providers and service meshes
3. Is supported by OpenTelemetry natively
4. Has a small overhead to avoid impacting request latency
5. Is easy to implement in both Rust and TypeScript

## Decision

We will use the W3C Trace Context standard (`traceparent`, `tracestate`) as the primary mechanism for propagating trace context between services. We will:
1. Implement full support for W3C Trace Context specification v1.0
2. Add optional support for legacy propagation formats (B3, Jaeger) as optional extensions only
3. Use the official OpenTelemetry context propagation APIs to handle header injection/extraction
4. Validate trace context format to prevent invalid propagation

## Consequences

- Out-of-the-box compatibility with all W3C-compliant services and proxies
- Seamless integration with OpenTelemetry collectors and backends
- Minimal overhead from trace headers (small payload size)
- Support for cross-vendor distributed tracing without custom configuration
- Users needing legacy format support must enable the optional extension
