#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -eq 1 ]; then
  archive_path="$1"
else
  echo "Usage: $0 ARCHIVE_PATH" >&2
  exit 2
fi

command -v cargo >/dev/null
command -v dirname >/dev/null
command -v mktemp >/dev/null
command -v sha256sum >/dev/null
command -v tar >/dev/null

case "$archive_path" in
  /*) ;;
  *) archive_path="$PWD/$archive_path" ;;
esac

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_dir="$(cd "$script_dir/.." && pwd)"
archive_dir="$(dirname "$archive_path")"
archive_name="${archive_path##*/}"
work_dir="$(mktemp -d)"
trap 'rm -rf "$work_dir"' EXIT

mkdir -p "$archive_dir" "$work_dir/.cargo"

(
  cd "$work_dir"
  cargo vendor --locked --versioned-dirs --manifest-path "$repo_dir/Cargo.toml" vendor \
    > .cargo/config.toml
)

tar -C "$work_dir" -czf "$archive_path" .cargo vendor
(
  cd "$archive_dir"
  sha256sum "$archive_name" > "$archive_name.sha256"
)
