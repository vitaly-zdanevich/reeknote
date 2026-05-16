#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 5 ]; then
  echo "Usage: $0 VERSION reeknote|rnsync amd64|arm64 BINARY_TARBALL OUTPUT_DIR" >&2
  exit 2
fi

version="$1"
snap_name="$2"
snap_arch="$3"
binary_tarball="$4"
output_dir="$5"
command_name="$snap_name"

if [[ ! "$version" =~ ^[0-9][A-Za-z0-9._+-]*$ ]]; then
  echo "Invalid snap package version: $version" >&2
  exit 2
fi

case "$snap_name" in
  reeknote | rnsync) ;;
  *)
    echo "Unsupported snap name: $snap_name" >&2
    exit 2
    ;;
esac

case "$snap_arch" in
  amd64 | arm64) ;;
  *)
    echo "Unsupported snap architecture: $snap_arch" >&2
    exit 2
    ;;
esac

if [ ! -f "$binary_tarball" ]; then
  echo "Binary tarball not found: $binary_tarball" >&2
  exit 2
fi

command -v snapcraft >/dev/null
command -v sha256sum >/dev/null
command -v tar >/dev/null

work_dir="$(mktemp -d)"
trap 'rm -rf "$work_dir"' EXIT

payload_dir="$work_dir/$snap_name"
mkdir -p "$payload_dir/bin" "$payload_dir/meta" "$payload_dir/usr/share/doc/$snap_name" "$output_dir"

tar -xzf "$binary_tarball" -C "$work_dir"
install -m 0755 "$work_dir/$command_name" "$payload_dir/bin/$command_name"
install -m 0644 README.md "$payload_dir/usr/share/doc/$snap_name/README.md"
install -m 0644 LICENSE "$payload_dir/usr/share/doc/$snap_name/LICENSE"

if [ "$snap_name" = "reeknote" ]; then
  title="Reeknote"
  summary="Command-line Evernote client"
  description="Reeknote is a command-line client for Evernote."
else
  title="rnsync"
  summary="Evernote note synchronization CLI"
  description="rnsync downloads Evernote notes to local files."
fi

cat > "$payload_dir/meta/snap.yaml" <<EOF
name: $snap_name
title: $title
version: '$version'
summary: $summary
description: |
  $description
license: GPL-3.0-only
type: app
base: core24
grade: stable
confinement: classic
architectures:
  - $snap_arch
apps:
  $snap_name:
    command: bin/$command_name
EOF

snapcraft pack "$payload_dir" --build-for "$snap_arch" --output "$output_dir"

generated_snap="$(find "$output_dir" -maxdepth 1 -type f -name "${snap_name}_${version}_${snap_arch}.snap" | sort | head -n 1)"
if [ -z "$generated_snap" ]; then
  echo "Built snap not found" >&2
  exit 1
fi

normalized_snap="$output_dir/${snap_name}_${version}_${snap_arch}.snap"
if [ "$generated_snap" != "$normalized_snap" ]; then
  mv "$generated_snap" "$normalized_snap"
fi

sha256sum "$normalized_snap" > "$normalized_snap.sha256"
