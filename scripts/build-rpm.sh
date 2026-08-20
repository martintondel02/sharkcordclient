#!/usr/bin/env bash
# Build an RPM of sharkcordclient for Fedora.
#
# Usage:
#   scripts/build-rpm.sh
#
# Produces: target/generate-rpm/sharkcordclient-<version>-<release>.<arch>.rpm
#
# Prereqs (Fedora):
#   cargo install cargo-generate-rpm
#   sudo dnf install mesa-libGL libX11 wayland-devel mesa-libEGL \
#                   freetype fontconfig libstdc++
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

echo "==> Building release binary..."
cargo build --release --package sharkcord-desktop

echo "==> Generating RPM..."
cargo generate-rpm --package sharkcord-desktop

RPM="$(ls target/generate-rpm/*.rpm 2>/dev/null | head -1)"
if [ -z "$RPM" ]; then
  echo "ERROR: no RPM produced" >&2
  exit 1
fi

echo
echo "RPM built: $RPM"
echo
echo "Install with: sudo dnf install \"$RPM\""
echo "Run with:     sharkcord-desktop"