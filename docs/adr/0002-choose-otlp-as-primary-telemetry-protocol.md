# 2. Choose OTLP as the primary telemetry export protocol

Date: 2025-11-01

## Status

Accepted

## Context

Request X-Ray needs to export telemetry data (traces, metrics, logs) to various backends including OpenTelemetry collectors, Jaeger, Zipkin, and cloud-native observability platforms. We need a protocol that:
1. Is widely adopted and supported by all major observability vendors
2. Supports all three telemetry signals (traces, metrics, logs)
3. Is efficient over the network (supports HTTP/1.1, HTTP/2, gRPC)
4. Has native client libraries in both Rust and TypeScript
5. Is the future standard for cloud-native observability

## Decision

We will use the OpenTelemetry Protocol (OTLP) as the sole primary protocol for exporting all telemetry data from Request X-Ray. We will:
1. Use the official OpenTelemetry Rust SDK (`opentelemetry`) for core library telemetry
2. Use the official OpenTelemetry JS SDK (`@opentelemetry/api`, `@opentelemetry/sdk-node`) for TypeScript SDKs
3. Implement OTLP exporters for both gRPC and HTTP/JSON to ensure compatibility
4. Avoid building custom exporters for legacy protocols (only add them as optional adapters if absolutely necessary)

## Consequences

- All Request X-Ray users will need an OTLP-compatible backend or collector
- We leverage official, maintained SDKs instead of building custom protocol implementations
- Automatic compatibility with all major cloud observability platforms (AWS X-Ray, GCP Cloud Trace, Azure Monitor)
- Future-proof against protocol changes as OTLP is the industry standard
- Smaller maintenance burden since we don't have to support multiple export protocols natively
