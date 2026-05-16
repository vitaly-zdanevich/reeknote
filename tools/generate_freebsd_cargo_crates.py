#!/usr/bin/env python3
import sys
import tomllib
from pathlib import Path


def main() -> None:
    if len(sys.argv) != 3:
        raise SystemExit(f"Usage: {sys.argv[0]} Cargo.lock Makefile.crates")

    lock_path = Path(sys.argv[1])
    output_path = Path(sys.argv[2])
    lock = tomllib.loads(lock_path.read_text())
    crates = [
        f"{package['name']}-{package['version']}"
        for package in lock.get("package", [])
        if str(package.get("source", "")).startswith("registry+")
    ]

    if not crates:
        raise SystemExit(f"No registry crates found in {lock_path}")

    lines = ["CARGO_CRATES=\t" + crates[0]]
    lines.extend(f"\t\t{crate}" for crate in crates[1:])
    output_path.write_text(" \\\n".join(lines) + "\n")


if __name__ == "__main__":
    main()
