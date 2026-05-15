#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 4 ]; then
  echo "Usage: $0 VERSION amd64|arm64 BINARY_TARBALL OUTPUT_DIR" >&2
  exit 2
fi

version="$1"
architecture="$2"
binary_tarball="$3"
output_dir="$4"

case "$architecture" in
  amd64 | arm64) ;;
  *)
    echo "Unsupported Debian architecture: $architecture" >&2
    exit 2
    ;;
esac

if [[ ! "$version" =~ ^[0-9][A-Za-z0-9.+:~_-]*$ ]]; then
  echo "Invalid Debian package version: $version" >&2
  exit 2
fi

if [ ! -f "$binary_tarball" ]; then
  echo "Binary tarball not found: $binary_tarball" >&2
  exit 2
fi

command -v dpkg-deb >/dev/null

mkdir -p "$output_dir"
work_dir="$(mktemp -d)"
trap 'rm -rf "$work_dir"' EXIT

tar -xzf "$binary_tarball" -C "$work_dir"

package_root="$work_dir/reeknote_${version}_${architecture}"
mkdir -p \
  "$package_root/DEBIAN" \
  "$package_root/usr/bin" \
  "$package_root/usr/share/doc/reeknote"

install -m 0755 "$work_dir/reeknote" "$package_root/usr/bin/reeknote"
install -m 0755 "$work_dir/rnsync" "$package_root/usr/bin/rnsync"
install -m 0644 README.md "$package_root/usr/share/doc/reeknote/README.md"
install -m 0644 LICENSE "$package_root/usr/share/doc/reeknote/copyright"

release_date="$(date -R)"
cat > "$work_dir/changelog.Debian" <<EOF
reeknote ($version) stable; urgency=medium

  * Release $version.

 -- Vitaly Zdanevich <zdanevich.vitaly@ya.ru>  $release_date
EOF
gzip -9 -n < "$work_dir/changelog.Debian" > "$package_root/usr/share/doc/reeknote/changelog.Debian.gz"

installed_size="$(du -ks "$package_root/usr" | cut -f1)"
cat > "$package_root/DEBIAN/control" <<EOF
Package: reeknote
Version: $version
Section: utils
Priority: optional
Architecture: $architecture
Maintainer: Vitaly Zdanevich <zdanevich.vitaly@ya.ru>
Installed-Size: $installed_size
Depends: libc6 (>= 2.36), ca-certificates
Recommends: mpv
Suggests: kitty
Homepage: https://gitlab.com/vitaly-zdanevich/reeknote
Description: command-line Evernote client
 Reeknote is a command-line client for Evernote.
 .
 It includes rnsync for downloading Evernote notes to local files.
EOF

package_path="$output_dir/reeknote_${version}_${architecture}.deb"
dpkg-deb --build --root-owner-group "$package_root" "$package_path"
sha256sum "$package_path" > "$package_path.sha256"
