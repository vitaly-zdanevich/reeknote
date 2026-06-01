#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -lt 2 ] || [ "$#" -gt 3 ]; then
  echo "Usage: $0 VERSION OUTPUT_DIR [source-only|build-rpm]" >&2
  exit 2
fi

version="$1"
output_dir="$2"
mode="${3:-source-only}"
package_name="reeknote"

if [[ ! "$version" =~ ^[0-9][A-Za-z0-9._+-]*$ ]]; then
  echo "Invalid OBS package version: $version" >&2
  exit 2
fi

case "$mode" in
  source-only | build-rpm) ;;
  *)
    echo "Invalid OBS package mode: $mode" >&2
    exit 2
    ;;
esac

if [ ! -f packaging/obs/reeknote.spec.in ]; then
  echo "Missing packaging/obs/reeknote.spec.in" >&2
  exit 2
fi

command -v cargo >/dev/null
command -v git >/dev/null
command -v sha256sum >/dev/null
command -v tar >/dev/null

work_dir="$(mktemp -d)"
trap 'rm -rf "$work_dir"' EXIT

mkdir -p "$output_dir"

source_archive="$output_dir/$package_name-$version.tar.gz"
vendor_archive="$output_dir/vendor-$version.tar.gz"
spec_file="$output_dir/$package_name.spec"

git archive --format=tar.gz --prefix="$package_name-$version/" -o "$source_archive" HEAD
bash tools/build_vendor_archive.sh "$vendor_archive"
sed "s|@VERSION@|$version|g" packaging/obs/reeknote.spec.in > "$spec_file"

sha256sum "$source_archive" > "$source_archive.sha256"

if [ "$mode" = "source-only" ]; then
  exit 0
fi

command -v find >/dev/null
command -v rpm >/dev/null
command -v rpmbuild >/dev/null

rpmbuild_dir="$work_dir/rpmbuild"
mkdir -p "$rpmbuild_dir/SOURCES" "$rpmbuild_dir/SPECS"
cp "$source_archive" "$vendor_archive" "$rpmbuild_dir/SOURCES/"
cp "$spec_file" "$rpmbuild_dir/SPECS/"

rpmbuild --define "_topdir $rpmbuild_dir" -ba "$rpmbuild_dir/SPECS/$package_name.spec"

rpm_arch="$(rpm --eval '%{_arch}')"
binary_rpm="$(find "$rpmbuild_dir/RPMS" -type f -name "$package_name-$version-*.rpm" | sort | head -n 1)"
source_rpm="$(find "$rpmbuild_dir/SRPMS" -type f -name "$package_name-$version-*.src.rpm" | sort | head -n 1)"

if [ -z "$binary_rpm" ]; then
  echo "Built OBS RPM not found" >&2
  exit 1
fi

if [ -z "$source_rpm" ]; then
  echo "Built OBS source RPM not found" >&2
  exit 1
fi

normalized_binary="$output_dir/${package_name}_${version}_opensuse_${rpm_arch}.rpm"
normalized_source="$output_dir/${package_name}_${version}_opensuse.src.rpm"
cp "$binary_rpm" "$normalized_binary"
cp "$source_rpm" "$normalized_source"
sha256sum "$normalized_binary" > "$normalized_binary.sha256"
sha256sum "$normalized_source" > "$normalized_source.sha256"
