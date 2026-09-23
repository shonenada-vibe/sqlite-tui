#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 || $# -gt 2 ]]; then
  echo "Usage: $0 <tag> [checksum-directory]  (e.g. v0.1.0)" >&2
  exit 1
fi

tag="$1"
if [[ ! "$tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "Error: expected a stable version tag such as v0.1.0" >&2
  exit 1
fi

version="${tag#v}"
checksum_directory="${2:-}"
base_url="https://github.com/shonenada-vibe/sqlite-tui/releases/download/${tag}"

fetch_sha256() {
  local target="$1"
  local archive="sqlite-tui-${tag}-${target}.tar.gz"
  local line sha filename extra
  if [[ -n "$checksum_directory" ]]; then
    line=$(cat "${checksum_directory}/${archive}.sha256")
  else
    line=$(curl --fail --silent --show-error --location --retry 5 --retry-delay 2 "${base_url}/${archive}.sha256")
  fi
  read -r sha filename extra <<< "$line"
  if [[ "$line" == *$'\n'* || ! "$sha" =~ ^[0-9a-f]{64}$ || "$filename" != "$archive" || -n "$extra" ]]; then
    echo "Error: invalid checksum for ${archive}" >&2
    return 1
  fi
  printf '%s\n' "$sha"
}

sha_aarch64_darwin=$(fetch_sha256 aarch64-apple-darwin)
sha_x86_64_darwin=$(fetch_sha256 x86_64-apple-darwin)
sha_x86_64_linux=$(fetch_sha256 x86_64-unknown-linux-gnu)

cat <<EOF
class SqliteTui < Formula
  desc "Keyboard-driven SQLite browser and SQL workbench"
  homepage "https://github.com/shonenada-vibe/sqlite-tui"
  version "${version}"

  on_macos do
    depends_on macos: :big_sur

    if Hardware::CPU.arm?
      url "${base_url}/sqlite-tui-${tag}-aarch64-apple-darwin.tar.gz"
      sha256 "${sha_aarch64_darwin}"
    elsif Hardware::CPU.intel?
      url "${base_url}/sqlite-tui-${tag}-x86_64-apple-darwin.tar.gz"
      sha256 "${sha_x86_64_darwin}"
    end
  end

  on_linux do
    depends_on arch: :x86_64

    if Hardware::CPU.intel?
      url "${base_url}/sqlite-tui-${tag}-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "${sha_x86_64_linux}"
    end
  end

  def install
    bin.install "sqlite-tui"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/sqlite-tui --version")
    assert_match "SQLite browser", shell_output("#{bin}/sqlite-tui --help")
  end
end
EOF
