# Security Policy

## Project status and supported versions

Request X-Ray is currently a **design/pre-release project**. No production-safe version has been published or designated as supported.

| Version | Security support |
| --- | --- |
| Pre-implementation / unreleased | No support guarantees |
| Future stable release | To be defined at release |

## Reporting vulnerabilities privately

**Preferred channel once enabled:** GitHub repository → **Security** → **Advisories** → **Report a vulnerability** (GitHub private vulnerability reporting).

**Maintainer action required:** Enable private vulnerability reporting in repository security settings and verify that the workflow works. Until that is confirmed, **do not claim that the channel is operational**. Maintainers should configure a dedicated private security contact before accepting external reports; record it in `docs/MAINTAINERS.md`.

Do not post security vulnerabilities, proof-of-concept exploits, credentials, or sensitive telemetry in public issues, discussions, or pull requests.

A useful report includes affected version/commit, environmental assumptions, reproducible steps with non-sensitive test data, expected versus actual behavior, impact, and a suggested mitigation where possible.

## Response process

When a valid private channel is configured, maintainers should acknowledge reports, triage severity, coordinate a fix and regression tests, prepare a security advisory for affected supported releases, and publish an appropriate patched version. Specific acknowledgement and remediation SLAs are **not yet promised**.

## Security priorities

- Default-deny sensitive telemetry capture and redact before persistence/export.
- Local-only bind by default; authentication and TLS are required for exposed deployments.
- Bound request sizes, decompression ratios, ingestion concurrency, retention and disk use.
- Ensure collector unavailability never breaks instrumented host applications.
- Avoid executing arbitrary recorded requests as part of replay.
- Scan dependencies and build artifacts; provide reproducible release provenance.

This policy does not imply a security audit or a production-readiness guarantee.
