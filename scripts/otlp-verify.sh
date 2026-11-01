#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET="$ROOT/schemas/vendor/opentelemetry-proto"
MANIFEST="$ROOT/schemas/otlp-sha256.txt"
EXPECTED_COMMIT="b3f75588eb23c5fca62264edd05d382de49beb1a"
EXPECTED_TAG="v1.11.1"
[[ -f "$TARGET/UPSTREAM_COMMIT" ]] || { echo 'Missing sources: run bash scripts/otlp-sync.sh' >&2; exit 1; }
[[ -f "$TARGET/UPSTREAM_TAG" ]] || { echo 'Missing tag metadata' >&2; exit 1; }
[[ "$(cat "$TARGET/UPSTREAM_COMMIT")" == "$EXPECTED_COMMIT" ]] || { echo 'Unexpected upstream commit' >&2; exit 1; }
[[ "$(cat "$TARGET/UPSTREAM_TAG")" == "$EXPECTED_TAG" ]] || { echo 'Unexpected upstream tag' >&2; exit 1; }
[[ -f "$MANIFEST" ]] || { echo 'Missing schemas/otlp-sha256.txt: regenerate trusted manifest following docs/OTLP_PROTOBUF.md' >&2; exit 1; }
(
  cd "$TARGET"
  shasum -a 256 -c "$MANIFEST"
)
echo 'Verified pinned OTLP revision and SHA-256 checksums.'
