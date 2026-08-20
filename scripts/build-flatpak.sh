#!/usr/bin/env bash
# Build a Flatpak of sharkcordclient for Fedora (and any Flatpak host).
#
# Usage:
#   scripts/build-flatpak.sh          # build + install the local bundle
#   scripts/build-flatpak.sh --bundle # also produce a .flatpak bundle file
#
# Prereqs (Fedora):
#   sudo dnf install flatpak flatpak-builder
#   flatpak remote-add --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo
#   flatpak install flathub org.freedesktop.Sdk//24.08 org.freedesktop.Platform//24.08
#   flatpak install flathub org.freedesktop.Sdk.Extension.rust-stable//24.08
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

MANIFEST="dist/flatpak/io.github.martintondel02.sharkcordclient.json"
APP_ID="io.github.martintondel02.sharkcordclient"
BUILD_DIR="${ROOT}/.flatpak-build"
REPO_DIR="${ROOT}/.flatpak-repo"

echo "==> Vendoring Cargo dependencies for offline Flatpak build..."
cargo vendor --workspace vendor 2>/dev/null || {
  echo "  (cargo vendor skipped; the manifest falls back to online build)"
}

# Point the manifest at the vendored sources by generating a vendor tarball.
if [ -d vendor ]; then
  echo "==> Generating vendored-sources tarball + cargo config..."
  # A cargo config that forces offline use of the vendor dir.
  mkdir -p .cargo
  cat > .cargo/config.toml <<'EOF'
[source.crates-io]
replace-with = "vendored-sources"

[source.vendored-sources]
directory = "vendor"
EOF
  tar -czf dist/flatpak/vendor.tar.gz -C vendor .
  echo "  done. (dist/flatpak/vendor.tar.gz + .cargo/config.toml)"
fi

echo "==> Building Flatpak (this may take a while on first run)..."
rm -rf "$BUILD_DIR"
flatpak-builder --force-clean --ccache --require-changes --repo="$REPO_DIR" \
  "$BUILD_DIR" "$MANIFEST"

echo "==> Installing the app into the user Flatpak..."
flatpak install --user --assumes --reinstall "$REPO_DIR" "$APP_ID" || true

if [ "${1:-}" = "--bundle" ]; then
  BUNDLE="${ROOT}/dist/${APP_ID}.flatpak"
  echo "==> Building bundle: $BUNDLE"
  flatpak build-bundle "$REPO_DIR" "$BUNDLE" "$APP_ID"
  echo "  done. Install with: flatpak install \"$BUNDLE\""
fi

echo
echo "Run with: flatpak run $APP_ID"