#!/usr/bin/env bash
set -euo pipefail

if [[ "$#" -ne 3 ]]; then
  echo 'Usage: bash scripts/verify-fresh-install.sh <CLI tarball> <platform tarball> <SDK tarball>' >&2
  exit 2
fi

archives=()
for archive in "$@"; do
  archives+=("$(cd "$(dirname "$archive")" && pwd)/$(basename "$archive")")
done
project=$(mktemp -d)
collector=''
cleanup() {
  if [[ -n "$collector" ]]; then
    kill "$collector" 2>/dev/null || true
    wait "$collector" 2>/dev/null || true
  fi
  rm -rf "$project"
}
trap cleanup EXIT
cd "$project"
export npm_config_cache="$project/npm-cache"
npm init -y >/dev/null
npm install --ignore-scripts --no-fund --no-audit --registry=https://registry.npmjs.org/ "${archives[@]}"
npx --offline request-xray --version
npx --offline request-xray init
node --input-type=module -e "import { startXRay } from '@request-xray/node'; if (typeof startXRay !== 'function') process.exit(1)"
node -e "if (typeof require('@request-xray/node').startXRay !== 'function') process.exit(1)"
node node_modules/request-xray/bin/request-xray.cjs demo --no-open --collector-port 0 --query-port 0 > demo.log 2>&1 &
collector=$!
for ((attempt=0; attempt<100; attempt++)); do
  if grep -q 'Local trace view:' demo.log; then break; fi
  kill -0 "$collector" || break
  sleep 0.1
done
cat demo.log
otlp=$(sed -nE 's@.*OTLP/HTTP listener at http://127\.0\.0\.1:([0-9]+)/.*@\1@p' demo.log)
query=$(sed -nE 's@.*Local trace view: http://127\.0\.0\.1:([0-9]+)/.*@\1@p' demo.log)
npx --offline request-xray --collector-port "$otlp" --query-port "$query" doctor
npx --offline request-xray --query-port "$query" traces list
node --input-type=module -e '
  const response = await fetch(process.argv[1], { signal: AbortSignal.timeout(5000) });
  const stats = await response.json();
  if (!response.ok || stats.spanCount !== 3) throw new Error(JSON.stringify(stats));
' "http://127.0.0.1:$query/api/v1/stats"
if [[ "${XRAY_AUDIT_PACKAGES:-0}" == 1 ]]; then
  npm audit --omit=dev --audit-level=high --registry=https://registry.npmjs.org/
fi
echo "Verified fresh install on $(node -p "process.platform + '/' + process.arch + ' Node ' + process.version")"
