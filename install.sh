#!/bin/sh
# Install the llmsort CLI from a prebuilt GitHub release binary.
#
#   curl -fsSL https://raw.githubusercontent.com/XyraSinclair/llmsort/main/install.sh | sh
#
# LLMSORT_INSTALL_DIR  where the binary goes (default: ~/.local/bin)
# LLMSORT_VERSION      a release tag to pin, e.g. v0.15.0 (default: latest)
set -eu

repo="XyraSinclair/llmsort"
dir="${LLMSORT_INSTALL_DIR:-$HOME/.local/bin}"

fail() {
    printf 'llmsort install: %s\n' "$1" >&2
    exit 1
}

case "$(uname -s)/$(uname -m)" in
    Darwin/arm64 | Darwin/aarch64) target=aarch64-apple-darwin ;;
    Darwin/x86_64) target=x86_64-apple-darwin ;;
    Linux/x86_64 | Linux/amd64) target=x86_64-unknown-linux-musl ;;
    Linux/aarch64 | Linux/arm64) target=aarch64-unknown-linux-musl ;;
    *) fail "no prebuilt binary for $(uname -s) $(uname -m); build it with: cargo install llmsort" ;;
esac

asset="llmsort-$target.tar.gz"
if [ -n "${LLMSORT_VERSION:-}" ]; then
    url="https://github.com/$repo/releases/download/$LLMSORT_VERSION/$asset"
else
    url="https://github.com/$repo/releases/latest/download/$asset"
fi

if command -v curl >/dev/null 2>&1; then
    fetch() { curl -fsSL "$1" -o "$2"; }
elif command -v wget >/dev/null 2>&1; then
    fetch() { wget -qO "$2" "$1"; }
else
    fail "needs curl or wget to download $url"
fi

if command -v sha256sum >/dev/null 2>&1; then
    digest() { sha256sum "$1"; }
elif command -v shasum >/dev/null 2>&1; then
    digest() { shasum -a 256 "$1"; }
else
    fail "needs sha256sum or shasum to verify the download"
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

fetch "$url" "$tmp/$asset" || fail "download failed: $url"
fetch "$url.sha256" "$tmp/$asset.sha256" || fail "download failed: $url.sha256"
want="$(cut -d ' ' -f 1 "$tmp/$asset.sha256")"
got="$(digest "$tmp/$asset" | cut -d ' ' -f 1)"
[ "$want" = "$got" ] || fail "checksum mismatch for $asset (want $want, got $got)"

tar -xzf "$tmp/$asset" -C "$tmp"
mkdir -p "$dir"
chmod 755 "$tmp/llmsort"
mv -f "$tmp/llmsort" "$dir/llmsort"

printf 'installed %s at %s\n' "$("$dir/llmsort" --version)" "$dir/llmsort"
case ":$PATH:" in
    *":$dir:"*) ;;
    *) printf 'note: %s is not on your PATH; add it to run llmsort by name\n' "$dir" ;;
esac
printf 'next: export OPENROUTER_API_KEY=... and try:\n  curl -fsSLO https://raw.githubusercontent.com/XyraSinclair/llmsort/main/examples/ideas.txt\n  llmsort sort ideas.txt --by "usefulness as startup advice" --scores\n'
