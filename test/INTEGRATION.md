# Root-repository integration notes

This directory is deliberately `test/` rather than `test/test/`. Extract the ZIP to `request-xray/` and keep its `test/` folder. Never install within the parent npm SDK's published package by accident.

## Recommended root scripts (optional)

Add scripts to the main repository only after inspecting its existing workspace tooling:

```json
{
  "scripts": {
    "lab:build": "npm --prefix test run build",
    "lab:run": "npm --prefix test run start:otel",
    "lab:smoke": "npm --prefix test run verify",
    "lab:scenarios": "npm --prefix test run scenario:all"
  }
}
```

## Future test assertion API

When collector query endpoints exist, add a separate non-flaky integration test that:

1. Starts an isolated collector with temporary SQLite and free ports.
2. Starts the two lab servers with `XRAY_MODE=otel` or `xray` and the collector URL.
3. Calls `/orders/checkout` and records the request correlation identifier.
4. Polls the collector query API with an explicit deadline until the trace arrives.
5. Asserts service names, span hierarchy, statuses, duration, sanitization and correlations.
6. Shuts down processes gracefully and deletes ephemeral data.

Do not hardcode collector query API routes until ARCHITECTURE.md APIs have been implemented. The OTLP HTTP receiver itself is `/v1/traces` in the intended specification.
