# Maintainers and Governance

**Status:** Proposed governance for an early-stage project. Ownership information below needs confirmation by the repository owner.

## Ownership and maintainer roster

| Role | GitHub handle | Responsibility | Status |
| --- | --- | --- | --- |
| Project owner / lead maintainer | **fahadsidd107** (Muhammad Fahad Siddiqui) | Roadmap, approvals, releases, package ownership | ✅ Recorded |
| Additional maintainer(s) | None assigned | Review, issue triage, security rotation | Not assigned |

The directory name on an individual developer's computer is **not** evidence of GitHub or npm ownership. Do not assign user handles, email addresses, or organizational rights without authorization.

## Governance model

Initially the project uses a **maintainer-led model**:

- Any contributor may propose issues, PRs, or design changes.
- Maintainers make final merge/release decisions following review and tests.
- Changes to public APIs, telemetry semantics, security posture, licensing or storage schema require documented review and preferably an ADR.
- A maintainer must not approve their own high-risk change without another reviewer when one is available; if sole-maintainer, document risk and test evidence.
- Maintainers can delegate scoped permissions through the hosting platform; npm publish access remains restricted.

## Review and merge policy

- Prefer at least one independent approving review once there are two or more maintainers.
- All configured required CI checks must pass; never bypass a failing security check for convenience.
- Breaking changes require compatibility notes, tests and migration documentation.
- Security-related patches may be developed privately before coordinated disclosure.
- Remove stale branches and avoid committing generated secrets or user telemetry.

## Reporting contacts — must be configured

- **Security:** enable GitHub private vulnerability reporting and test the private advisory flow. See `SECURITY.md`.
- **Conduct:** add a dedicated private contact (for example a maintainers-only address) before public community intake. **Not yet configured.**
- **General support:** public issues and discussions once repository settings allow them.

Do not announce public vulnerability/contact addresses until they exist and have been validated.

## Access, continuity and succession

- Require 2FA for owners and publishers; use least-privilege GitHub and npm permissions.
- Publish via trusted publishing/short-lived credentials where supported.
- Keep package and repository ownership coordinated and documented.
- When additional maintainers are added, record responsibility, review access and an emergency access plan.
- For a dormant project, communicate archival/maintenance status rather than implying continued support.

## Final owner checklist

- [x] Confirm exact GitHub account/organization that owns `request-xray` - @fahadsidd107
- [ ] Confirm actual npm package ownership and scope rights.
- [ ] Add lead maintainer's public handle (with permission).
- [ ] Configure and test private security reporting.
- [ ] Configure a private code-of-conduct contact.
- [ ] Protect the default branch and set required checks when CI exists.
- [ ] Configure npm publishing access, 2FA and provenance.