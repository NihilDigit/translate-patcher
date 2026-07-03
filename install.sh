#!/usr/bin/env bash
set -euo pipefail

repo="NihilDigit/translate-patcher"
install_dir="${INSTALL_DIR:-$HOME/.local/bin}"

os="$(uname -s)"
arch="$(uname -m)"

case "$os:$arch" in
  Linux:x86_64) target="x86_64-unknown-linux-musl"; ext="tar.gz" ;;
  Linux:aarch64|Linux:arm64) target="aarch64-unknown-linux-musl"; ext="tar.gz" ;;
  *) echo "unsupported platform: $os $arch" >&2; exit 1 ;;
esac

api="https://api.github.com/repos/$repo/releases/latest"
tag="$(curl -fsSL "$api" | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -1)"

if [ -z "$tag" ]; then
  echo "could not find latest release for $repo" >&2
  exit 1
fi

asset="translate-patcher-${tag}-${target}.${ext}"
base="https://github.com/$repo/releases/download/$tag"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

curl -fsSL "$base/$asset" -o "$tmp/$asset"
curl -fsSL "$base/checksums-${target}.txt" -o "$tmp/checksums.txt"

cd "$tmp"
sha256sum -c checksums.txt

mkdir -p "$install_dir"
tar -xzf "$asset"
install -m 0755 translate-patcher "$install_dir/translate-patcher"

echo "installed translate-patcher $tag to $install_dir/translate-patcher"

