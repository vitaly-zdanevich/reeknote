#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -lt 2 ] || [ "$#" -gt 3 ]; then
  echo "Usage: $0 reeknote|rnsync SNAP_DIR [CHANNEL]" >&2
  exit 2
fi

snap_name="$1"
snap_dir="$2"
channel="${3:-stable}"

case "$snap_name" in
  reeknote | rnsync) ;;
  *)
    echo "Unsupported snap name: $snap_name" >&2
    exit 2
    ;;
esac

if [ ! -d "$snap_dir" ]; then
  echo "Missing snap directory: $snap_dir" >&2
  exit 2
fi

if [ -z "${SNAPCRAFT_STORE_CREDENTIALS:-}" ]; then
  echo "SNAPCRAFT_STORE_CREDENTIALS must be set to publish snaps" >&2
  exit 2
fi

if [ -f "$SNAPCRAFT_STORE_CREDENTIALS" ]; then
  SNAPCRAFT_STORE_CREDENTIALS="$(cat "$SNAPCRAFT_STORE_CREDENTIALS")"
  export SNAPCRAFT_STORE_CREDENTIALS
fi

command -v snapcraft >/dev/null

shopt -s nullglob
snap_files=("$snap_dir/${snap_name}_"*.snap)
shopt -u nullglob

if [ "${#snap_files[@]}" -eq 0 ]; then
  echo "No snap files found for $snap_name in $snap_dir" >&2
  exit 1
fi

snapcraft whoami

for snap_file in "${snap_files[@]}"; do
  snapcraft upload --release="$channel" "$snap_file"
done
