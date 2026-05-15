#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 1 ]; then
  echo "Usage: $0 VERSION" >&2
  exit 2
fi

version="$1"
package_name="reeknote"
project_url="${CI_PROJECT_URL:-https://gitlab.com/vitaly-zdanevich/reeknote}"
source_url="${AUR_SOURCE_URL:-$project_url/-/archive/$version/reeknote-$version.tar.gz}"
aur_remote="ssh://aur@aur.archlinux.org/$package_name.git"
work_dir="$(mktemp -d)"
aur_repo="$(mktemp -d -t "aur-$package_name.XXXXXX")"
ssh_agent_started=0

cleanup() {
  rm -rf "$work_dir" "$aur_repo"
  if [ "$ssh_agent_started" = "1" ]; then
    ssh-agent -k >/dev/null
  fi
}
trap cleanup EXIT

if [[ ! "$version" =~ ^[0-9]+[.][0-9]+[.][0-9]+$ ]]; then
  echo "Invalid AUR package version: $version" >&2
  exit 2
fi

if [ ! -f aur/PKGBUILD.template ]; then
  echo "Missing aur/PKGBUILD.template" >&2
  exit 2
fi

command -v curl >/dev/null
command -v git >/dev/null
command -v makepkg >/dev/null
command -v ssh-add >/dev/null
command -v ssh-agent >/dev/null
command -v ssh-keyscan >/dev/null

archive_path="$work_dir/reeknote-$version.tar.gz"
curl --fail --location --show-error --silent "$source_url" --output "$archive_path"
source_sha256="$(sha256sum "$archive_path" | cut -d ' ' -f 1)"

generated_pkgbuild="$work_dir/PKGBUILD"
sed \
  -e "s|@VERSION@|$version|g" \
  -e "s|@SOURCE_URL@|$source_url|g" \
  -e "s|@SHA256@|$source_sha256|g" \
  aur/PKGBUILD.template > "$generated_pkgbuild"

install -d -m 0700 "$HOME/.ssh"

if [ -n "${AUR_SSH_KNOWN_HOSTS:-}" ]; then
  if [ -f "$AUR_SSH_KNOWN_HOSTS" ]; then
    cp "$AUR_SSH_KNOWN_HOSTS" "$HOME/.ssh/known_hosts"
  else
    printf '%s\n' "$AUR_SSH_KNOWN_HOSTS" > "$HOME/.ssh/known_hosts"
  fi
else
  ssh-keyscan aur.archlinux.org > "$HOME/.ssh/known_hosts"
fi
chmod 0600 "$HOME/.ssh/known_hosts"

if [ -z "${AUR_SSH_PRIVATE_KEY:-}" ]; then
  echo "AUR_SSH_PRIVATE_KEY is required" >&2
  exit 2
fi

ssh_key_path="$work_dir/aur_ssh_key"
if [ -f "$AUR_SSH_PRIVATE_KEY" ]; then
  cp "$AUR_SSH_PRIVATE_KEY" "$ssh_key_path"
else
  printf '%s\n' "$AUR_SSH_PRIVATE_KEY" | tr -d '\r' > "$ssh_key_path"
fi
chmod 0600 "$ssh_key_path"

eval "$(ssh-agent -s)"
ssh_agent_started=1
ssh-add "$ssh_key_path"

if ! git -c init.defaultBranch=master clone "$aur_remote" "$aur_repo"; then
  mkdir -p "$aur_repo"
  git -C "$aur_repo" -c init.defaultBranch=master init
  git -C "$aur_repo" remote add origin "$aur_remote"
fi

cp "$generated_pkgbuild" "$aur_repo/PKGBUILD"

if [ "$(id -u)" -eq 0 ]; then
  aur_build_user="${AUR_BUILD_USER:-aurbuild}"
  if ! id -u "$aur_build_user" >/dev/null 2>&1; then
    useradd --create-home "$aur_build_user"
  fi
  chown -R "$aur_build_user:$aur_build_user" "$aur_repo"
  su "$aur_build_user" -s /bin/bash -c "cd '$aur_repo' && makepkg --printsrcinfo > .SRCINFO"
  chown -R "$(id -u):$(id -g)" "$aur_repo"
else
  (
    cd "$aur_repo"
    makepkg --printsrcinfo > .SRCINFO
  )
fi

git -C "$aur_repo" config user.name "${GITLAB_USER_NAME:-Reeknote CI}"
git -C "$aur_repo" config user.email "${GITLAB_USER_EMAIL:-ci@gitlab.com}"
git -C "$aur_repo" add PKGBUILD .SRCINFO

if git -C "$aur_repo" diff --cached --quiet; then
  echo "AUR package $package_name is already up to date for $version."
  exit 0
fi

git -C "$aur_repo" commit -m "Update to $version"
git -C "$aur_repo" push origin HEAD:master
