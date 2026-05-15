#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 2 ]; then
  echo "Usage: $0 DEB_DIR PUBLIC_DIR" >&2
  exit 2
fi

deb_dir="$1"
public_dir="$2"

command -v dpkg-scanpackages >/dev/null
command -v gpg >/dev/null
command -v gzip >/dev/null
command -v python3 >/dev/null

if [ "${APT_REPO_UNSIGNED:-}" != "1" ]; then
  if [ -z "${APT_GPG_PRIVATE_KEY:-}" ]; then
    echo "APT_GPG_PRIVATE_KEY must be set to sign the APT repository" >&2
    exit 2
  fi
fi

rm -rf "$public_dir"
mkdir -p \
  "$public_dir/pool/main/r/reeknote" \
  "$public_dir/dists/stable/main/binary-amd64" \
  "$public_dir/dists/stable/main/binary-arm64"

cp "$deb_dir"/reeknote_*_amd64.deb "$public_dir/pool/main/r/reeknote/"
cp "$deb_dir"/reeknote_*_arm64.deb "$public_dir/pool/main/r/reeknote/"

for architecture in amd64 arm64; do
  packages_path="dists/stable/main/binary-$architecture/Packages"
  (
    cd "$public_dir"
    dpkg-scanpackages --arch "$architecture" pool > "$packages_path"
    gzip -9 -n < "$packages_path" > "$packages_path.gz"
  )
done

python3 tools/write_apt_release.py "$public_dir/dists/stable"

if [ "${APT_REPO_UNSIGNED:-}" = "1" ]; then
  exit 0
fi

gpg_home="$(mktemp -d)"
trap 'rm -rf "$gpg_home"' EXIT
chmod 700 "$gpg_home"
export GNUPGHOME="$gpg_home"

if [ -f "$APT_GPG_PRIVATE_KEY" ]; then
  gpg --batch --import "$APT_GPG_PRIVATE_KEY"
else
  printf '%s' "$APT_GPG_PRIVATE_KEY" | gpg --batch --import
fi

gpg --batch --armor --export > "$public_dir/reeknote-archive-keyring.asc"

passphrase_args=()
if [ -n "${APT_GPG_PASSPHRASE:-}" ]; then
  passphrase_args=(--pinentry-mode loopback --passphrase "$APT_GPG_PASSPHRASE")
fi

gpg --batch --yes "${passphrase_args[@]}" \
  --clearsign \
  --output "$public_dir/dists/stable/InRelease" \
  "$public_dir/dists/stable/Release"

gpg --batch --yes "${passphrase_args[@]}" \
  --detach-sign \
  --armor \
  --output "$public_dir/dists/stable/Release.gpg" \
  "$public_dir/dists/stable/Release"
