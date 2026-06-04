#!/bin/sh
set -eu

repo="${ISSUE_FINDER_REPO:-lifuyue/issue-finder}"
version="${ISSUE_FINDER_VERSION:-latest}"
install_dir="${ISSUE_FINDER_INSTALL_DIR:-$HOME/.local/bin}"

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "issue-finder installer: missing required command: $1" >&2
    exit 1
  fi
}

need_cmd curl
need_cmd tar

os="$(uname -s)"
arch="$(uname -m)"

case "$os:$arch" in
  Darwin:arm64 | Darwin:aarch64)
    asset="issue-finder-aarch64-apple-darwin.tar.gz"
    ;;
  Darwin:x86_64)
    asset="issue-finder-x86_64-apple-darwin.tar.gz"
    ;;
  Linux:x86_64 | Linux:amd64)
    asset="issue-finder-x86_64-unknown-linux-gnu.tar.gz"
    ;;
  *)
    echo "issue-finder installer: unsupported platform: $os $arch" >&2
    echo "Download a release manually from https://github.com/$repo/releases" >&2
    exit 1
    ;;
esac

if [ "$version" = "latest" ]; then
  base_url="https://github.com/$repo/releases/latest/download"
else
  base_url="https://github.com/$repo/releases/download/$version"
fi

tmp_dir="$(mktemp -d)"
cleanup() {
  rm -rf "$tmp_dir"
}
trap cleanup EXIT INT TERM

archive="$tmp_dir/$asset"
checksums="$tmp_dir/SHA256SUMS"

echo "Downloading Issue Finder $version for $os $arch..."
curl -fsSL "$base_url/$asset" -o "$archive"

if command -v sha256sum >/dev/null 2>&1 || command -v shasum >/dev/null 2>&1; then
  curl -fsSL "$base_url/SHA256SUMS" -o "$checksums"
  checksum_line="$(grep " $asset\$" "$checksums" || true)"
  if [ -z "$checksum_line" ]; then
    echo "issue-finder installer: SHA256SUMS does not include $asset" >&2
    exit 1
  fi
  if command -v sha256sum >/dev/null 2>&1; then
    printf '%s\n' "$checksum_line" | (cd "$tmp_dir" && sha256sum -c -)
  else
    printf '%s\n' "$checksum_line" | (cd "$tmp_dir" && shasum -a 256 -c -)
  fi
else
  echo "issue-finder installer: sha256sum or shasum not found; skipping checksum verification" >&2
fi

tar -xzf "$archive" -C "$tmp_dir"

if [ ! -f "$tmp_dir/issue-finder" ]; then
  echo "issue-finder installer: archive did not contain an issue-finder executable" >&2
  exit 1
fi

mkdir -p "$install_dir"
cp "$tmp_dir/issue-finder" "$install_dir/issue-finder"
chmod 755 "$install_dir/issue-finder"

echo "Installed issue-finder to $install_dir/issue-finder"

case ":$PATH:" in
  *":$install_dir:"*) ;;
  *)
    echo "Add $install_dir to your PATH to run issue-finder from any shell."
    ;;
esac
