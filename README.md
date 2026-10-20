# shellcheck shell=bash
# Shared setup for the packaging scripts. Source it: `. "$(dirname "$0")/../env.sh"`.
# Adapted from PhotoCraft's packaging (storytold/photocraft, Apache-2.0; see ATTRIBUTION.md).
#
# Exports:
#   ROOT                    workspace root
#   VERSION                 [workspace.package] version from Cargo.toml (override: SOLIDCRAFT_VERSION)
#
# Every SOLIDCRAFT_* override also accepts the pre-rename SOLVECRAFT_* name when the new one is
# unset (compatibility shim, see docs/compatibility.md).
#   DIST                    output directory for release artifacts (default: $ROOT/dist/release)
#   SOLIDCRAFT_BUILD_DATE   UTC build date, YYYY-MM-DD
#   CARGO_TARGET_DIR        cargo's target dir (default: $ROOT/target)

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export ROOT

workspace_version() {
  awk '
    /^\[/ { in_pkg = ($0 == "[workspace.package]") ; next }
    in_pkg && $1 == "version" { gsub(/[" ]/, "", $3); print $3; exit }
  ' "$ROOT/Cargo.toml"
}

VERSION="${SOLIDCRAFT_VERSION:-${SOLVECRAFT_VERSION:-$(workspace_version)}}"
if [ -z "$VERSION" ]; then
  echo "error: could not read [workspace.package] version from $ROOT/Cargo.toml" >&2
  exit 1
fi
export VERSION

DIST="${DIST:-$ROOT/dist/release}"
mkdir -p "$DIST"
DIST="$(cd "$DIST" && pwd)"
export DIST

export SOLIDCRAFT_BUILD_DATE="${SOLIDCRAFT_BUILD_DATE:-${SOLVECRAFT_BUILD_DATE:-$(date -u +%Y-%m-%d)}}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"
# Custom app id for OmniSynced. Previous SolidCraft names remain available as compatibility shims.
APP_ID=ai.storyteller.omni-synced
export APP_ID

warn() {
  if [ -n "${GITHUB_ACTIONS:-}" ]; then echo "::warning::$*"; else echo "warning: $*" >&2; fi
}

copy_docs() {
  local dest="$1" f
  for f in README.md LICENSE-MIT LICENSE-APACHE NOTICE THIRD-PARTY-LICENSES.txt; do
    if [ -f "$ROOT/$f" ]; then cp "$ROOT/$f" "$dest/"; fi
  done
}

make_icons() {
  local out="$2" s
  for s in 16 24 32 48 64 128 256 512; do
    mkdir -p "$out/hicolor/${s}x${s}/apps"
    cp "$ROOT/assets/icons/app/linux/exports/hicolor/${s}x${s}/apps/$APP_ID.png" "$out/hicolor/${s}x${s}/apps/$APP_ID.png" || return
  done
  cp "$ROOT/assets/icons/app/windows/exports/solidcraft.ico" "$out/solidcraft.ico"
}
