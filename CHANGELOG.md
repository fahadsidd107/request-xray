# Changelog

## 0.1.0 (unreleased)

- Add a loopback OTLP/HTTP protobuf collector and read-only trace query API backed by SQLite.
- Add a Node 22/24 SDK with HTTP instrumentation, W3C context propagation and bounded, sanitized export.
- Add a native CLI and a browser trace explorer with filters, a waterfall, span inspection and a synthetic demo.
- Package the native CLI for macOS ARM64 and x64, Linux ARM64 and x64 with glibc 2.35 or newer, and Windows x64. The launcher verifies the installed executable checksum before running it.

The alpha supports local development workflows. See the [CLI](docs/CLI.md) and [Node SDK](docs/NODE_SDK.md) guides for setup and limitations.
