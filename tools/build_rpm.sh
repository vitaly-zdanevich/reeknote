#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -lt 2 ] || [ "$#" -gt 3 ]; then
  echo "Usage: $0 VERSION OUTPUT_DIR [include-srpm|binary-only]" >&2
  exit 2
fi

version="$1"
output_dir="$2"
srpm_mode="${3:-include-srpm}"
package_name="reeknote"

if [[ ! "$version" =~ ^[0-9][A-Za-z0-9._+-]*$ ]]; then
  echo "Invalid RPM package version: $version" >&2
  exit 2
fi

case "$srpm_mode" in
  include-srpm | binary-only) ;;
  *)
    echo "Invalid SRPM mode: $srpm_mode" >&2
    exit 2
    ;;
esac

if [ ! -f packaging/fedora/reeknote.spec.in ]; then
  echo "Missing packaging/fedora/reeknote.spec.in" >&2
  exit 2
fi

command -v cargo >/dev/null
command -v git >/dev/null
command -v rpmbuild >/dev/null
command -v rpm >/dev/null
command -v sha256sum >/dev/null
command -v tar >/dev/null

work_dir="$(mktemp -d)"
trap 'rm -rf "$work_dir"' EXIT

rpmbuild_dir="$work_dir/rpmbuild"
sources_dir="$rpmbuild_dir/SOURCES"
specs_dir="$rpmbuild_dir/SPECS"
mkdir -p "$sources_dir" "$specs_dir" "$output_dir"

source_archive="$sources_dir/$package_name-$version.tar.gz"
vendor_archive="$sources_dir/vendor-$version.tar.gz"
spec_file="$specs_dir/$package_name.spec"

git archive --format=tar.gz --prefix="$package_name-$version/" -o "$source_archive" HEAD
bash tools/build_vendor_archive.sh "$vendor_archive"
sed "s|@VERSION@|$version|g" packaging/fedora/reeknote.spec.in > "$spec_file"

rpmbuild --define "_topdir $rpmbuild_dir" -ba "$spec_file"

rpm_arch="$(rpm --eval '%{_arch}')"
binary_rpm="$(find "$rpmbuild_dir/RPMS" -type f -name "$package_name-$version-*.rpm" | sort | head -n 1)"

if [ -z "$binary_rpm" ]; then
  echo "Built RPM not found" >&2
  exit 1
fi

normalized_binary="$output_dir/${package_name}_${version}_${rpm_arch}.rpm"
cp "$binary_rpm" "$normalized_binary"
sha256sum "$normalized_binary" > "$normalized_binary.sha256"

if [ "$srpm_mode" = "include-srpm" ]; then
  source_rpm="$(find "$rpmbuild_dir/SRPMS" -type f -name "$package_name-$version-*.src.rpm" | sort | head -n 1)"
  if [ -z "$source_rpm" ]; then
    echo "Built source RPM not found" >&2
    exit 1
  fi

  normalized_source="$output_dir/${package_name}_${version}.src.rpm"
  cp "$source_rpm" "$normalized_source"
  sha256sum "$normalized_source" > "$normalized_source.sha256"
fi
