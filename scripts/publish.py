"""Upload verified artifacts; never replace existing release bytes."""

import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import zipfile

import package


def run(*args):
    return subprocess.check_output(args, text=True).strip()


def main():
    tag = os.environ["RELEASE_TAG"]
    if tag != f"v{package.version()}":
        raise ValueError("Release tag and Cargo version differ")
    if run("git", "rev-parse", f"{tag}^{{commit}}") != run("git", "rev-parse", "HEAD"):
        raise ValueError("Publish only artifacts verified at the release commit")
    dist = Path("dist")
    original_checksums = (dist / "SHA256SUMS").read_bytes()
    package.metadata(dist, dist / "packages")
    if (dist / "SHA256SUMS").read_bytes() != original_checksums:
        raise ValueError("Release archive checksums changed")
    bundle = dist / f"cretspec-{package.version()}-packages.zip"
    with zipfile.ZipFile(bundle, "w", compression=zipfile.ZIP_DEFLATED) as archive:
        for path in sorted((dist / "packages").rglob("*")):
            if path.is_file():
                item = zipfile.ZipInfo(path.relative_to(dist / "packages").as_posix(), (1980, 1, 1, 0, 0, 0))
                item.compress_type = zipfile.ZIP_DEFLATED
                archive.writestr(item, path.read_bytes())
    existing = json.loads(run("gh", "release", "view", tag, "--json", "assets"))["assets"]
    names = {asset["name"] for asset in existing}
    assets = [dist / package.archive_name(package.version(), target) for target in package.TARGETS]
    assets.extend([dist / "SHA256SUMS", bundle])
    for asset in assets:
        if asset.name in names:
            with tempfile.TemporaryDirectory() as temporary:
                run("gh", "release", "download", tag, "--pattern", asset.name, "--dir", temporary)
                previous = Path(temporary, asset.name).read_bytes()
                if hashlib.sha256(previous).digest() != hashlib.sha256(asset.read_bytes()).digest():
                    raise ValueError(f"Existing release asset differs: {asset.name}")
        else:
            run("gh", "release", "upload", tag, str(asset))


if __name__ == "__main__":
    main()
