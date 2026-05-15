#!/usr/bin/env python3
import email.utils
import hashlib
import sys
from pathlib import Path


HASH_SECTIONS = [
    ("MD5Sum", "md5"),
    ("SHA1", "sha1"),
    ("SHA256", "sha256"),
    ("SHA512", "sha512"),
]


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit(f"Usage: {sys.argv[0]} DIST_DIR")

    dist_dir = Path(sys.argv[1])
    if not dist_dir.is_dir():
        raise SystemExit(f"Distribution directory not found: {dist_dir}")

    index_files = sorted(
        path
        for path in dist_dir.rglob("*")
        if path.is_file() and path.name in {"Packages", "Packages.gz"}
    )
    if not index_files:
        raise SystemExit(f"No APT index files found below {dist_dir}")

    lines = [
        "Origin: Reeknote",
        "Label: Reeknote",
        "Suite: stable",
        "Codename: stable",
        f"Date: {email.utils.formatdate(usegmt=True)}",
        "Architectures: amd64 arm64",
        "Components: main",
        "Description: Reeknote APT repository",
    ]

    for title, algorithm in HASH_SECTIONS:
        lines.append(f"{title}:")
        for path in index_files:
            content = path.read_bytes()
            digest = hashlib.new(algorithm, content).hexdigest()
            relative_path = path.relative_to(dist_dir).as_posix()
            lines.append(f" {digest} {len(content):16d} {relative_path}")

    (dist_dir / "Release").write_text("\n".join(lines) + "\n")


if __name__ == "__main__":
    main()
