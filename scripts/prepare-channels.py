"""Prepare catalog submissions from an immutable, published CretSpec release."""

import argparse
import hashlib
from pathlib import Path
import re
import urllib.request

import package


def prepare(release, output):
    if not re.fullmatch(r"(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)", release):
        raise ValueError("Expected a stable release version, for example 0.4.0")
    base = f"{package.REPOSITORY}/releases/download/v{release}"
    checksums = urllib.request.urlopen(f"{base}/SHA256SUMS", timeout=60).read().decode()
    expected = dict(line.split()[::-1] for line in checksums.splitlines() if line.strip())
    archives = output / "archives"
    archives.mkdir(parents=True, exist_ok=True)
    for target in package.TARGETS:
        name = package.archive_name(release, target)
        data = urllib.request.urlopen(f"{base}/{name}", timeout=60).read()
        if hashlib.sha256(data).hexdigest() != expected[name]:
            raise ValueError(f"Published checksum mismatch: {name}")
        (archives / name).write_bytes(data)
    package.metadata(archives, output / "packages", release)
    print(f"Prepared verified v{release} catalog files in {output / 'packages'}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True)
    parser.add_argument("--out", type=Path, default=Path("dist/channels"))
    args = parser.parse_args()
    prepare(args.version, args.out)
