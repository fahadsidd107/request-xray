# Native CLI and npm launcher

The npm launcher supports Node 22 and 24. Run from the workspace with `cargo run --locked -p xray-cli -- dev --no-open`. With an installed tarball pair, run `npx --offline request-xray dev --no-open`. The native executable starts the OTLP and query listeners in one process. The npm launcher selects an installed platform package and verifies its executable against the package SHA-256 before launching it.

| Command | Behavior |
| --- | --- |
| `version`, `--version` | Print the native CLI version. |
| `serve` | Start the collector and query API. |
| `dev [--no-open]` | Start both listeners and serve the browser dashboard of saved traces. |
| `doctor [--json]` | Probe query and SQLite read readiness, then check that the collector accepts an empty protobuf export. |
| `init [--dry-run]` | Generate project config and a CommonJS or ESM preload. Keep existing files and leave package scripts untouched. |
| `traces [list]` | List a paged set of trace summaries. Supports `--service`, `--route`, `--error`, nanosecond time bounds, `--limit`, `--cursor`, and `--json`. |
| `inspect <trace-id>`, `traces show <trace-id>` | Read a trace and one page of spans. Use `--tree` to show parent relationships and links, or `--json` for scriptable output. |
| `demo [--no-open]` | Load a labeled synthetic trace through OTLP into a temporary database. Remove the database on graceful shutdown. |
| `config validate [--file FILE]` | Validate effective configuration and print its listener settings. |

The packaged CLI serves the React dashboard. A source-only build without compiled dashboard assets retains the compact trace view. In an interactive terminal, `dev` and `demo` try to open the printed URL. With redirected output or `--no-open`, they print the URL and leave browser startup to the caller. [Dashboard build and use](DASHBOARD.md)

Both listeners bind to loopback. Port 0 chooses a free port, printed at startup. A port already in use produces a specific error and leaves the database untouched. The launcher closes a private stdin pipe on shutdown so the native process can stop admitting work, drain active requests and close SQLite. The native executable also handles Ctrl-C and Unix SIGTERM.

`doctor` checks reachability and read readiness. Verify write durability by sending a real application span and querying it. Use the [Node SDK guide](NODE_SDK.md) to connect the generated preload to an application.

## Configuration

The load order is built-in defaults, `request-xray.config.json`, environment, then explicit flags. Select a file with `--config` or `REQUEST_XRAY_CONFIG`. For validation, `--file` selects that command's file. A relative `databasePath` in a file is resolved relative to that file. Environment and CLI paths are relative to the working directory. Validation checks the [JSON fields](../packages/cli/config.schema.json), loopback addresses, and listener conflicts. Errors avoid printing unknown field names or values.

| Project field | Environment override | Flag or default |
| --- | --- | --- |
| `collectorListen` | `REQUEST_XRAY_OTLP_LISTEN`, then `REQUEST_XRAY_COLLECTOR_PORT` | `--collector-listen` or `--collector-port`, defaults to `127.0.0.1:4318`. |
| `queryListen` | `REQUEST_XRAY_QUERY_LISTEN`, then `REQUEST_XRAY_DASHBOARD_PORT` | `--query-listen` or `--query-port`, defaults to `127.0.0.1:4731`. |
| `databasePath` | `REQUEST_XRAY_DB_PATH` | `--db-path`, defaults to `.request-xray/traces.sqlite`. |
| `retentionAgeSeconds`, `retentionMaxBytes` | `REQUEST_XRAY_RETENTION_AGE_SECONDS`, `REQUEST_XRAY_RETENTION_MAX_BYTES` | Seven days and a 500 MiB periodic target. |
| `maxCompressedBytes`, `maxDecodedBytes`, `maxInFlight` | `REQUEST_XRAY_MAX_COMPRESSED_BYTES`, `REQUEST_XRAY_MAX_DECODED_BYTES`, `REQUEST_XRAY_MAX_IN_FLIGHT` | 4 MiB, 16 MiB, and eight admitted requests. |
| `bodyTimeoutMs`, `sinkTimeoutMs` | `REQUEST_XRAY_BODY_TIMEOUT_MS`, `REQUEST_XRAY_SINK_TIMEOUT_MS` | 10 seconds each. |
| `allowAttributes`, `denyAttributes` | `REQUEST_XRAY_ALLOW_ATTRIBUTES`, `REQUEST_XRAY_DENY_ATTRIBUTES` | Built-in denials remain enabled. Environment lists are comma-separated. |

Exit codes are 0 for success, 1 for a runtime failure, 2 for invalid usage or config, 3 for unavailable local services, port collisions or missing traces, and 4 for an unsupported, missing or damaged native package.

## Build and test npm tarballs

The [packaging workflow](../.github/workflows/npm-cli.yml) builds and smoke-tests macOS ARM64 and x64, Linux ARM64 and x64 with glibc 2.35 or newer, and Windows x64. It uploads tarballs as CI artifacts. Registry publication remains a separate approval. [GitHub-hosted runner reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)

Build from a supported host. Set `MACOSX_DEPLOYMENT_TARGET=11.0` on macOS when building release artifacts.

```sh
corepack pnpm install --frozen-lockfile
corepack pnpm --filter @request-xray/query-client build
corepack pnpm --filter @request-xray/dashboard build
XRAY_REQUIRE_DASHBOARD=1 cargo build --locked --release -p xray-cli
node scripts/stage-npm-cli.cjs target/release/request-xray-native npm-cli-stage
XRAY_REQUIRE_DASHBOARD=1 node scripts/smoke-npm-cli.cjs npm-cli-stage
```

On Windows, set `$env:XRAY_REQUIRE_DASHBOARD = "1"` in PowerShell and use `target/release/request-xray-native.exe`. The staging directory must be new. The smoke script packs both packages, installs them into a clean npm project with lifecycle scripts disabled, restricts the runtime PATH to Node and required system tools, and exercises help, version, init, config precedence, doctor, port collisions, trace inspection, demo, and shutdown. It also checks missing packages and a modified native executable.

Install the generated `request-xray-0.1.0.tgz` and the platform tarball together in a test project. The launcher tarball names each exact-version platform package as an npm optional dependency. Install the matching local tarball until the packages are published. Platform packages declare their operating system and CPU, plus glibc on Linux. The packages contain no install scripts and the launcher makes no download calls. The checksum detects damaged files within the installed package.

For example, install both macOS ARM64 tarballs from a staging directory into a clean project:

```sh
npm install --offline --ignore-scripts /path/to/stage/request-xray-0.1.0.tgz \
  /path/to/stage/request-xray-cli-darwin-arm64-0.1.0.tgz
npx --offline request-xray --version
npx --offline request-xray demo --no-open --collector-port 0 --query-port 0
```

## Verification status

The installed tarball smoke passed on macOS ARM64 and macOS x64 on 2026-11-23, including the x64 binary and Node runtime under Rosetta. Linux ARM64 and x64 passed in glibc 2.36 containers, with x64 running under emulation. The verified artifacts are in the local `target/npm-cli-tarballs` directory. The smoke tests exclude Cargo from PATH and use an offline npm install.

On 2026-11-25, the dashboard-enabled macOS ARM64 tarballs passed the expanded smoke test. It checks the installed launcher entry point, exact optional dependency versions, platform metadata, absence of install hooks, and exit code 4 for a missing package, a mismatched version, or a modified executable.

On 2026-11-25, GitHub Actions passed the installed-tarball smoke on all five targets, including Windows x64. The jobs built release binaries with the embedded dashboard, installed both tarballs in an offline npm project, and uploaded the packaged artifacts. [Packaging CI run](https://github.com/fahadsidd107/request-xray/actions/runs/45649415386)
