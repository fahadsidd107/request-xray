#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO="https://github.com/open-telemetry/opentelemetry-proto.git"
TAG="v1.11.1"
COMMIT="b3f75588eb23c5fca62264edd05d382de49beb1a"
TARGET="$ROOT/schemas/vendor/opentelemetry-proto"
MANIFEST="$ROOT/schemas/otlp-sha256.txt"
if [[ -e "$TARGET" || -e "$MANIFEST" ]]; then
  echo "Refusing to overwrite existing vendored sources or checksum manifest." >&2
  echo "For a reviewed refresh, remove both schemas/vendor/opentelemetry-proto and schemas/otlp-sha256.txt." >&2
  exit 1
fi
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
git clone --quiet --depth 1 --branch "$TAG" "$REPO" "$TMP/upstream"
ACTUAL="$(git -C "$TMP/upstream" rev-parse HEAD)"
[[ "$ACTUAL" == "$COMMIT" ]] || { echo "Pinned revision mismatch: $ACTUAL" >&2; exit 1; }
mkdir -p "$TARGET"
cp "$TMP/upstream/LICENSE" "$TARGET/LICENSE"
FILES=(
  opentelemetry/proto/common/v1/common.proto
  opentelemetry/proto/resource/v1/resource.proto
  opentelemetry/proto/trace/v1/trace.proto
  opentelemetry/proto/collector/trace/v1/trace_service.proto
)
for file in "${FILES[@]}"; do
  mkdir -p "$TARGET/$(dirname "$file")"
  cp "$TMP/upstream/$file" "$TARGET/$file"
done
printf '%s\n' "$COMMIT" > "$TARGET/UPSTREAM_COMMIT"
printf '%s\n' "$TAG" > "$TARGET/UPSTREAM_TAG"
# Store checksums independently of the vendored directory. All paths are relative to vendor root.
(
  cd "$TARGET"
  shasum -a 256 LICENSE UPSTREAM_COMMIT UPSTREAM_TAG "${FILES[@]}" | LC_ALL=C sort -k2
) > "$MANIFEST"
echo "Vendored official OTLP trace protobuf inputs at $COMMIT"
echo "Wrote checksums to schemas/otlp-sha256.txt"
