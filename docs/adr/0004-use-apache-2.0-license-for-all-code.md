# 4. Use Apache 2.0 license for all code

Date: 2025-11-01

## Status

Accepted

## Context

Request X-Ray will be an open-source project that needs to balance:
1. Commercial use and adoption by companies of all sizes
2. Protection of contributors' intellectual property
3. Compatibility with other popular open-source licenses (MIT, BSD, GPL)
4. Patent protection for contributors and users
5. Clarity on trademark usage and attribution

## Decision

We will license all Request X-Ray code under the Apache License 2.0. We will:
1. Include the full Apache 2.0 license text in the repository root
2. Add copyright headers to all source files (when we start writing code)
3. Add a NOTICE file that lists contributors and attribution requirements
4. Ensure all third-party dependencies are compatible with Apache 2.0
5. Maintain a CLA (Contributor License Agreement) for external contributions to protect the project

## Consequences

- Companies can use Request X-Ray in commercial products without open-sourcing their own code
- Patent grants from all contributors protect users from patent litigation
- Compatibility with most other open-source licenses allows integration into larger projects
- The license is widely understood and accepted by the open-source community
- We avoid the copyleft requirements of GPL licenses that may limit commercial adoption
