# Steps 0.1 + 0.2 — Installation and sign-off

This bundle contains the **missing Step 0.1** record and the **Step 0.2** governance package. It is intended to be extracted over an existing Request X-Ray repository **without replacing** your README, ARCHITECTURE, ROADMAP, or application code.

## File locations

```text
request-xray/                       ← repository root
├── LICENSE                         ← 0.2 Apache-2.0 license
├── CODE_OF_CONDUCT.md              ← 0.2 community behavior
├── CONTRIBUTING.md                 ← 0.2 contributor guide
├── SECURITY.md                     ← 0.2 private vulnerability process
├── GOVERNANCE_SETUP.md             ← 0.1 + 0.2 installation / decisions
├── docs/
│   ├── NAME_VERIFICATION.md        ← 0.1 naming and ownership record
│   ├── MAINTAINERS.md              ← 0.2 maintainers / contacts
│   └── RELEASE_POLICY.md           ← 0.2 versioning and publication
└── .github/
    ├── PULL_REQUEST_TEMPLATE.md    ← 0.2 pull-request checklist
    └── ISSUE_TEMPLATE/
        ├── bug_report.yml          ← 0.2 bug report form
        ├── feature_request.yml     ← 0.2 feature form
        └── config.yml              ← 0.2 issue picker config
```

## Installation

From the directory **containing** the current `request-xray` Git repository, unzip the archive:

```bash
unzip request-xray-step-0.1-and-0.2.zip
```

Or if you're already **inside** the root, extract to a temp location and copy the contents of its `request-xray/` directory into `.`:

```bash
mkdir -p /tmp/request-xray-governance
unzip ~/Downloads/request-xray-step-0.1-and-0.2.zip -d /tmp/request-xray-governance
cp -R /tmp/request-xray-governance/request-xray/. .
```

Existing files with the same names will be overwritten by the copy command: review first if you have modified them.

## Step 0.1 — owner action required

Read `docs/NAME_VERIFICATION.md`. Confirm live npm name/scope/publish permissions, exact GitHub ownership, and trademark checks. Domains may be deferred. Record evidence and **do not claim verified ownership merely from search results**.

## Step 0.2 — owner action required

- [ ] Confirm Apache-2.0 is your desired license and ensure copyright notices are accurate.
- [ ] Add the actual maintainer's GitHub handle to `docs/MAINTAINERS.md`.
- [ ] Configure the community conduct private contact in `CODE_OF_CONDUCT.md` and related docs.
- [ ] Enable GitHub private vulnerability reporting or provide a tested confidential reporting channel.
- [ ] Configure issue labels `bug` and `enhancement`; enable GitHub issues.
- [ ] Confirm npm publish permissions and establish release approvals.
- [ ] Run checks below and review the resulting files before committing.

## Verification and completion

```bash
git status --short
git diff --check
find docs .github -type f | sort
head -n 3 LICENSE
```

Only mark Step 0.1 and Step 0.2 as complete in `ROADMAP.md` after their respective checklist items have been verified and signed off. This bundle does **not** silently modify ROADMAP.md.
