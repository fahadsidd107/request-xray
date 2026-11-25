# Request X-Ray — Name and Ownership Verification (Step 0.1)

**Status: IN PROGRESS — not yet verified or reserved.**  
**Record date:** 2026-11-01  
**Proposed public brand:** Request X-Ray  
**Proposed repository slug:** `request-xray`

This is a factual verification record and checklist, **not a certificate of availability**. Search-engine results, an unregistered local folder, and npm `404` responses do **not** prove future availability, scope ownership, or freedom from trademark conflicts.

## Candidate identifiers

| Candidate | Check required | Current status | Ownership evidence |
| --- | --- | --- | --- |
| npm unscoped `request-xray` | npm registry + npm website + npm CLI availability on authenticated account | **Unconfirmed** | Not reserved or verified |
| npm scope `@request-xray` | npm owner or organization creation and permissions | **Unconfirmed** | Not reserved or verified |
| `@request-xray/node` | Package name and permission to publish in scope | **Unconfirmed** | Not reserved or verified |
| `@request-xray/cli` | Same | **Unconfirmed** | Not reserved or verified |
| `@request-xray/dashboard` | Same | **Unconfirmed** | Not reserved or verified |
| `@request-xray/core` | Same | **Unconfirmed** | Not reserved or verified |
| GitHub repository `request-xray` | **Under the selected user or organization** | **Unconfirmed** | Local directory is not proof |
| `requestxray.dev` | Registrar availability and checkout | **Unconfirmed** | Not purchased |
| `request-xray.dev` | Registrar availability and checkout | **Unconfirmed** | Not purchased |

> Important npm nuance: `@request-xray` is a **scope**. A name not returned by an npm package search is not evidence that you own the scope. The actual publishing identity and organization configuration must be checked using the authenticated npm account.

## How to verify — repeat immediately before publishing

1. Open <https://www.npmjs.com/package/request-xray> and the scoped package pages. Check from the official registry as well. An unpublished package does not imply the name is guaranteed to remain free.
2. Run `npm whoami` and check which npm user/organization will own packages. Confirm the `request-xray` scope can actually be created or used by that account. Obtain and record the owner organization's explicit permission before publishing.
3. Run `npm view request-xray name version --json` and `npm view @request-xray/node name version --json`. If the request returns 404, record **not found at check time**, not **registered/owned**.
4. Check <https://github.com/OWNER/request-xray> under the **actual selected GitHub owner** and create the repository to reserve the slug in that namespace. The same repository slug can be used by different owners.
5. Run a conflict review for **Request X-Ray**, **RequestXRay**, **Request Xray**, **XRAY**, and similar marks in official trademark databases relevant to distribution (e.g. <https://www.dpma.de>, <https://www.euipo.europa.eu>, <https://www.uspto.gov>). Check overlapping developer-tool/observability products and visually/sound-alike names. A preliminary search cannot rule out trademark claims; seek legal review if commercially important.
6. Query domain availability at a reputable registrar, and purchase before writing **owned**. Domain search snippets do not reserve domains.
7. Record the exact check timestamp, official URL, account used, result, reviewer, and any screenshot/evidence reference in the log below.

## Preliminary findings (not clearance)

- A broad web search performed on **2026-11-01** did not reliably establish ownership or registration status for the proposed npm identifiers or repository slug. **All remain unconfirmed**.
- The term **X-RAY** is used in existing trademark registrations and applications across software-related categories. This does not itself prove that **Request X-Ray** infringes, but the draft claim of “no active trademarks” is **not supported**. Start with official EU/German/US checks before approving the brand.
- No domain registration was performed.

## Verification log

| Checked at (UTC) | Identifier | Authoritative source URL | Result | Account/owner | Evidence | Reviewed by |
| --- | --- | --- | --- | --- | --- | --- |
| TBD | `request-xray` on npm | https://registry.npmjs.org/request-xray | Pending | TBD | TBD | TBD |
| TBD | `@request-xray` scope/publish permission | https://www.npmjs.com/ | Pending | TBD | TBD | TBD |
| TBD | GitHub repo slug | https://github.com/OWNER/request-xray | Pending | TBD | TBD | TBD |
| TBD | Trademark checks | Official DPMA/EUIPO/USPTO sources | Pending | TBD | TBD | TBD |
| TBD | Optional domains | Chosen domain registrar | Pending | TBD | TBD | TBD |

## Naming decision

- **Working name:** Request X-Ray
- **Published name:** **NOT FINALIZED**
- **Scope owner:** **TBD**
- **GitHub owner:** **TBD**
- **Domain:** Deferred; not owned
- **Trademark clearance:** Not established

### Sign-off criteria

- [ ] npm identifiers checked live and scope publishing permissions verified
- [ ] Repository created under the intended GitHub owner and URL recorded
- [ ] Trademark collision review recorded without unsupported assurances
- [ ] Domain recorded as owned, unavailable, or explicitly deferred
- [ ] Public package names and documentation examples updated if renamed
- [ ] Maintainer signs off on the chosen package owner and name

**Do not mark ROADMAP Step 0.1 complete before these checks and ownership decisions are recorded.**

## npm release preflight on 2026-11-25

The public npm registry returned HTTP 404 for each of the seven planned names at 16:45 UTC:

- `request-xray` and `@request-xray/node`
- `@request-xray/cli-darwin-arm64` and `@request-xray/cli-darwin-x64`
- `@request-xray/cli-linux-arm64-gnu` and `@request-xray/cli-linux-x64-gnu`
- `@request-xray/cli-win32-x64`

The workspace's npm CLI reported `ENEEDAUTH`. The owner still needs to verify control of the `request-xray` npm scope, publisher permissions for all seven names and first-publication authentication. A later GitHub check confirmed that [fahadsidd107/request-xray](https://github.com/fahadsidd107/request-xray) is public and the connected GitHub account has admin access.
