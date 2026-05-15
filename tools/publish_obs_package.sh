#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 2 ]; then
  echo "Usage: $0 VERSION OBS_SOURCE_DIR" >&2
  exit 2
fi

version="$1"
source_dir_input="$2"
project="${OBS_PROJECT:-home:vitaly-zdanevich:reeknote}"
package="${OBS_PACKAGE:-reeknote}"
api_url="${OBS_APIURL:-https://api.opensuse.org}"

if [[ ! "$version" =~ ^[0-9][A-Za-z0-9._+-]*$ ]]; then
  echo "Invalid OBS package version: $version" >&2
  exit 2
fi

if [ ! -d "$source_dir_input" ]; then
  echo "Missing OBS source directory: $source_dir_input" >&2
  exit 2
fi

source_dir="$(cd "$source_dir_input" && pwd)"

for file in \
  "$source_dir/reeknote.spec" \
  "$source_dir/reeknote-$version.tar.gz" \
  "$source_dir/vendor-$version.tar.gz"
do
  if [ ! -f "$file" ]; then
    echo "Missing OBS source file: $file" >&2
    exit 2
  fi
done

command -v osc >/dev/null

work_dir="$(mktemp -d)"
trap 'rm -rf "$work_dir"' EXIT

cd "$work_dir"
osc -A "$api_url" checkout "$project" "$package"
package_dir="$work_dir/$project/$package"

if [ ! -d "$package_dir" ]; then
  echo "OBS checkout did not create expected package directory: $package_dir" >&2
  exit 1
fi

find "$package_dir" -maxdepth 1 -type f \( \
  -name 'reeknote.spec' -o \
  -name 'reeknote-*.tar.gz' -o \
  -name 'vendor-*.tar.gz' \
\) -delete

cp "$source_dir/reeknote.spec" "$package_dir/"
cp "$source_dir/reeknote-$version.tar.gz" "$package_dir/"
cp "$source_dir/vendor-$version.tar.gz" "$package_dir/"

cd "$package_dir"
osc addremove

status="$(osc status)"
if [ -z "$status" ]; then
  echo "OBS package $project/$package is already up to date for $version."
  exit 0
fi

printf '%s\n' "$status"
osc commit -m "Update reeknote to $version"
