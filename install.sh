#!/bin/sh
# Installs blaze, ember, forge, spark and ward from the latest GitHub release.
set -eu
REPO="Max-imalgutaussehend/blaze"
BIN_DIR="${BLAZE_INSTALL_DIR:-$HOME/.local/bin}"

case "$(uname -s)-$(uname -m)" in
  Darwin-arm64)  TARGET=aarch64-apple-darwin ;;
  Darwin-x86_64) TARGET=x86_64-apple-darwin ;;
  Linux-x86_64)  TARGET=x86_64-unknown-linux-gnu ;;
  *) echo "blaze: no prebuilt binary for $(uname -s)-$(uname -m); try: cargo install --git https://github.com/$REPO blaze" >&2; exit 1 ;;
esac

URL="https://github.com/$REPO/releases/latest/download/blaze-$TARGET.tar.gz"
TMP="$(mktemp -d)"; trap 'rm -rf "$TMP"' EXIT
echo "downloading $URL"
curl -LsSf "$URL" -o "$TMP/blaze.tar.gz"
tar -xzf "$TMP/blaze.tar.gz" -C "$TMP"
mkdir -p "$BIN_DIR"
for b in blaze ember forge spark ward; do install -m 755 "$TMP/blaze-$TARGET/$b" "$BIN_DIR/$b"; done
echo "installed to $BIN_DIR"
case ":$PATH:" in *":$BIN_DIR:"*) ;; *) echo "note: add $BIN_DIR to your PATH" ;; esac
