#!/usr/bin/env python3
import argparse
import base64
import hashlib
import re
import zipfile
from pathlib import Path


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--package", required=True)
    parser.add_argument("--command", required=True)
    parser.add_argument("--binary", required=True)
    parser.add_argument("--platform-tag", required=True)
    parser.add_argument("--summary", required=True)
    parser.add_argument("--out", required=True)
    args = parser.parse_args()

    version = cargo_version(Path("Cargo.toml"))
    out_dir = Path(args.out)
    out_dir.mkdir(parents=True, exist_ok=True)

    distribution = normalize_distribution(args.package)
    dist_info = f"{distribution}-{version}.dist-info"
    data_dir = f"{distribution}-{version}.data"
    wheel_name = f"{distribution}-{version}-py3-none-{args.platform_tag}.whl"
    wheel_path = out_dir / wheel_name
    records: list[tuple[str, str, str]] = []

    with zipfile.ZipFile(wheel_path, "w", compression=zipfile.ZIP_DEFLATED) as wheel:
        add_bytes(
            wheel,
            records,
            f"{data_dir}/scripts/{args.command}",
            Path(args.binary).read_bytes(),
            0o755,
        )
        add_text(wheel, records, f"{dist_info}/METADATA", metadata(args, version), 0o644)
        add_text(
            wheel,
            records,
            f"{dist_info}/WHEEL",
            wheel_metadata(args.platform_tag),
            0o644,
        )
        add_text(wheel, records, f"{dist_info}/RECORD", record_contents(records, dist_info), 0o644)

    print(wheel_path)


def cargo_version(path: Path) -> str:
    match = re.search(r'^version\s*=\s*"([^"]+)"', path.read_text(), re.MULTILINE)
    if not match:
        raise SystemExit("Cargo.toml package version not found")
    return match.group(1)


def normalize_distribution(name: str) -> str:
    return re.sub(r"[^\w\d.]+", "_", name).strip("_")


def metadata(args: argparse.Namespace, version: str) -> str:
    readme = Path("README.md").read_text()
    return f"""Metadata-Version: 2.1
Name: {args.package}
Version: {version}
Summary: {args.summary}
License: GPL-3.0-only
Requires-Python: >=3.8
Home-page: https://github.com/vitaly-zdanevich/reeknote
Project-URL: Source, https://github.com/vitaly-zdanevich/reeknote
Description-Content-Type: text/markdown

{readme}
"""


def wheel_metadata(platform_tag: str) -> str:
    return f"""Wheel-Version: 1.0
Generator: reeknote-build-cli-wheel
Root-Is-Purelib: false
Tag: py3-none-{platform_tag}
"""


def add_text(
    wheel: zipfile.ZipFile,
    records: list[tuple[str, str, str]],
    archive_path: str,
    content: str,
    mode: int,
) -> None:
    add_bytes(wheel, records, archive_path, content.encode(), mode)


def add_bytes(
    wheel: zipfile.ZipFile,
    records: list[tuple[str, str, str]],
    archive_path: str,
    content: bytes,
    mode: int,
) -> None:
    info = zipfile.ZipInfo(archive_path)
    info.create_system = 3
    info.date_time = (1980, 1, 1, 0, 0, 0)
    info.compress_type = zipfile.ZIP_DEFLATED
    info.external_attr = (0o100000 | (mode & 0o777)) << 16
    wheel.writestr(info, content)
    records.append((archive_path, f"sha256={sha256_digest(content)}", str(len(content))))


def sha256_digest(content: bytes) -> str:
    digest = hashlib.sha256(content).digest()
    return base64.urlsafe_b64encode(digest).rstrip(b"=").decode()


def record_contents(records: list[tuple[str, str, str]], dist_info: str) -> str:
    lines = [",".join(row) for row in records]
    lines.append(f"{dist_info}/RECORD,,")
    return "\n".join(lines) + "\n"


if __name__ == "__main__":
    main()
