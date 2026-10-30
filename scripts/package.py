"""Build release archives and package metadata from the same verified bytes."""

import argparse
import gzip
import hashlib
import io
import json
from pathlib import Path
import re
import subprocess
import tarfile
import tomllib
import zipfile

ROOT = Path(__file__).resolve().parents[1]
REPOSITORY = "https://github.com/yveslaurentcreton/CretSpec"
TARGETS = (
    "x86_64-pc-windows-msvc",
    "x86_64-unknown-linux-gnu",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
)
# DLLs that a clean Windows installation may lack: the Visual C++ runtime and
# the Universal CRT forwarders.
RUNTIME_DLL = re.compile(r"(vcruntime|msvcp|api-ms-win-crt-|ucrtbase)", re.IGNORECASE)
# The WinGet catalog rejects deprecated manifest versions; scripts/validate-winget.ps1
# checks this version against its schema.
WINGET_MANIFEST_VERSION = "1.28.0"


def version():
    value = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
    if not re.fullmatch(r"(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)", value):
        raise ValueError("Package channels require a stable SemVer version")
    return value


def archive_name(release, target):
    if target not in TARGETS:
        raise ValueError("Unsupported release target")
    return f"cretspec-{release}-{target}." + ("zip" if "windows" in target else "tar.gz")


def archive_bytes(target, contents):
    result = io.BytesIO()
    if "windows" in target:
        with zipfile.ZipFile(result, "w", compression=zipfile.ZIP_DEFLATED) as archive:
            for name, data in sorted(contents.items()):
                item = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
                item.compress_type = zipfile.ZIP_DEFLATED
                item.external_attr = 0o100644 << 16
                archive.writestr(item, data)
    else:
        with gzip.GzipFile(fileobj=result, mode="wb", mtime=0, filename="") as compressed:
            with tarfile.open(fileobj=compressed, mode="w") as archive:
                for name, data in sorted(contents.items()):
                    item = tarfile.TarInfo(name)
                    item.size = len(data)
                    item.mode = 0o755 if name == "cspec" else 0o644
                    archive.addfile(item, io.BytesIO(data))
    return result.getvalue()


def pack(target, binary, output):
    release = version()
    reported = subprocess.check_output([str(binary.resolve()), "--version"], text=True).strip()
    if reported != f"cspec {release}":
        raise ValueError(f"Binary version mismatch: {reported}")
    executable = binary.read_bytes()
    if "windows" in target:
        runtime = sorted(name for name in pe_imports(executable) if RUNTIME_DLL.match(name))
        if runtime:
            raise ValueError("Link the C runtime statically; cspec.exe imports " + ", ".join(runtime))
    contents = {"cspec.exe" if "windows" in target else "cspec": executable}
    contents.update({name: (ROOT / name).read_bytes() for name in ("LICENSE", "README.md")})
    contents["THIRD-PARTY.txt"] = dependency_notices(target)
    toolchain = Path(subprocess.check_output(["rustc", "--print", "sysroot"], cwd=ROOT, text=True).strip())
    contents["RUST-LICENSES.html"] = (toolchain / "share/doc/rust/COPYRIGHT-library.html").read_bytes()
    output.mkdir(parents=True, exist_ok=True)
    destination = output / archive_name(release, target)
    destination.write_bytes(archive_bytes(target, contents))
    print(destination)


def pe_imports(data):
    """Return the DLL names in a PE32+ executable's import table."""
    def u16(offset):
        return int.from_bytes(data[offset:offset + 2], "little")

    def u32(offset):
        return int.from_bytes(data[offset:offset + 4], "little")

    header = u32(0x3C)
    if data[:2] != b"MZ" or data[header:header + 4] != b"PE\0\0":
        raise ValueError("Not a PE executable")
    coff = header + 4
    optional = coff + 20
    if u16(optional) != 0x20B:
        raise ValueError("Not a PE32+ executable")
    import_rva = u32(optional + 112 + 8)
    sections = [(u32(entry + 12), max(u32(entry + 8), u32(entry + 16)), u32(entry + 20))
                for entry in range(optional + u16(coff + 16), optional + u16(coff + 16) + 40 * u16(coff + 2), 40)]

    def offset(rva):
        for address, size, raw in sections:
            if address <= rva < address + size:
                return raw + rva - address
        raise ValueError("Import table outside the sections")

    names = []
    descriptor = offset(import_rva) if import_rva else None
    while descriptor is not None and any(data[descriptor:descriptor + 20]):
        start = offset(u32(descriptor + 12))
        names.append(data[start:data.index(b"\0", start)].decode("ascii"))
        descriptor += 20
    return names


def dependency_notices(target):
    data = json.loads(subprocess.check_output([
        "cargo", "metadata", "--locked", "--format-version", "1", "--filter-platform", target,
    ], cwd=ROOT, text=True))
    selected = {node["id"] for node in data["resolve"]["nodes"]}
    sections = ["CretSpec third-party dependency notices\n\nIncludes build dependencies for completeness.\n"]
    for crate in sorted(data["packages"], key=lambda item: (item["name"], item["version"])):
        if crate["id"] not in selected or not crate["source"]:
            continue
        root = Path(crate["manifest_path"]).parent
        notices = sorted(path for path in root.iterdir() if path.is_file() and path.name.upper().startswith(
            ("LICENSE", "LICENCE", "COPYING", "NOTICE", "COPYRIGHT", "UNLICENSE")))
        if not notices:
            raise ValueError(f"Review missing license texts for {crate['name']}")
        sections.append(f"\n{'=' * 72}\n{crate['name']} {crate['version']}\nLicense: {crate['license']}\n")
        for notice in notices:
            sections.append(f"\n--- {notice.name} ---\n{notice.read_text(encoding='utf-8')}\n")
    return "".join(sections).encode("utf-8")


def inspect_archive(path, target):
    binary = "cspec.exe" if "windows" in target else "cspec"
    expected = {binary, "LICENSE", "README.md", "THIRD-PARTY.txt", "RUST-LICENSES.html"}
    if "windows" in target:
        with zipfile.ZipFile(path) as archive:
            if set(archive.namelist()) != expected or len(archive.infolist()) != len(expected):
                raise ValueError(f"Unexpected archive contents: {path}")
            if not archive.read(binary):
                raise ValueError("Empty executable")
            if archive.testzip():
                raise ValueError("Corrupt archive")
    else:
        with tarfile.open(path, "r:gz") as archive:
            members = archive.getmembers()
            if {m.name for m in members} != expected or len(members) != len(expected):
                raise ValueError(f"Unexpected archive contents: {path}")
            if not all(m.isfile() for m in members):
                raise ValueError("Archive must contain only regular files")
            executable = archive.getmember(binary)
            if not executable.mode & 0o111 or executable.size == 0:
                raise ValueError("Missing executable permission or content")
    return hashlib.sha256(path.read_bytes()).hexdigest()


def metadata(artifacts, output, release=None):
    release = release or version()
    if not re.fullmatch(r"(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)", release):
        raise ValueError("Invalid stable version")
    names = {target: archive_name(release, target) for target in TARGETS}
    # Validate the complete matrix before emitting any metadata.
    hashes = {target: inspect_archive(artifacts / name, target) for target, name in names.items()}
    urls = {target: f"{REPOSITORY}/releases/download/v{release}/{name}" for target, name in names.items()}
    output.mkdir(parents=True, exist_ok=True)
    (artifacts / "SHA256SUMS").write_text("".join(f"{hashes[t]}  {names[t]}\n" for t in TARGETS), encoding="utf-8", newline="\n")
    formula = f'''class Cretspec < Formula
  desc "Prepare a complete development project from its specification"
  homepage "{REPOSITORY}"
  license "MIT"

  depends_on "git"
  depends_on :macos

  on_macos do
    if Hardware::CPU.arm?
      url "{urls[TARGETS[3]]}"
      sha256 "{hashes[TARGETS[3]]}"
    else
      url "{urls[TARGETS[2]]}"
      sha256 "{hashes[TARGETS[2]]}"
    end
  end

  def install
    bin.install "cspec"
    pkgshare.install "LICENSE", "README.md", "THIRD-PARTY.txt", "RUST-LICENSES.html"
  end

  test do
    assert_equal "cspec #{{version}}", shell_output("#{{bin}}/cspec --version").strip
    assert_match "project", shell_output("#{{bin}}/cspec --help")
  end
end
'''
    write(output / "homebrew/Formula/cretspec.rb", formula)
    identifier = "YvesLaurentCreton.CretSpec"
    common = {"PackageIdentifier": identifier, "PackageVersion": release}
    documents = {
        "": dict(common, DefaultLocale="en-US", ManifestType="version"),
        ".locale.en-US": dict(common, PackageLocale="en-US", Publisher="Yves-Laurent Creton",
            PackageName="CretSpec", License="MIT", LicenseUrl=f"{REPOSITORY}/blob/v{release}/LICENSE",
            ShortDescription="Prepare a complete development project from its specification",
            PackageUrl=REPOSITORY, Moniker="cspec", ManifestType="defaultLocale"),
        # A portable zip installer exposes its command through PortableCommandAlias.
        ".installer": dict(common, InstallerType="zip", NestedInstallerType="portable",
            NestedInstallerFiles=[{"RelativeFilePath": "cspec.exe", "PortableCommandAlias": "cspec"}],
            Dependencies={"PackageDependencies": [{"PackageIdentifier": "Git.Git"}]},
            UpgradeBehavior="uninstallPrevious", Installers=[{
                "Architecture": "x64", "InstallerUrl": urls[TARGETS[0]],
                "InstallerSha256": hashes[TARGETS[0]].upper()}], ManifestType="installer"),
    }
    for suffix, data in documents.items():
        data["ManifestVersion"] = WINGET_MANIFEST_VERSION
        header = f"# yaml-language-server: $schema=https://aka.ms/winget-manifest.{data['ManifestType']}.{WINGET_MANIFEST_VERSION}.schema.json\n\n"
        write(output / f"winget/manifests/y/YvesLaurentCreton/CretSpec/{release}/{identifier}{suffix}.yaml",
              header + "".join(line + "\n" for line in yaml_lines(data)))
    source = urls[TARGETS[1]]
    checksum = hashes[TARGETS[1]]
    write(output / "aur/PKGBUILD", f'''pkgname=cretspec-bin
pkgver={release}
pkgrel=1
pkgdesc='Prepare a complete development project from its specification'
arch=('x86_64')
url='{REPOSITORY}'
license=('MIT')
depends=('git' 'glibc' 'gcc-libs')
provides=('cretspec={release}')
conflicts=('cretspec')
options=('!strip')
source=("{source}")
sha256sums=('{checksum}')

package() {{
  install -Dm755 cspec "$pkgdir/usr/bin/cspec"
  install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
  install -Dm644 README.md "$pkgdir/usr/share/doc/$pkgname/README.md"
  install -Dm644 THIRD-PARTY.txt "$pkgdir/usr/share/licenses/$pkgname/THIRD-PARTY.txt"
  install -Dm644 RUST-LICENSES.html "$pkgdir/usr/share/licenses/$pkgname/RUST-LICENSES.html"
}}
''')
    write(output / "aur/.SRCINFO", f'''pkgbase = cretspec-bin
\tpkgdesc = Prepare a complete development project from its specification
\tpkgver = {release}
\tpkgrel = 1
\turl = {REPOSITORY}
\tarch = x86_64
\tlicense = MIT
\tdepends = git
\tdepends = glibc
\tdepends = gcc-libs
\tprovides = cretspec={release}
\tconflicts = cretspec
\toptions = !strip
\tsource = {source}
\tsha256sums = {checksum}

pkgname = cretspec-bin
''')


def yaml_lines(mapping, indent=0):
    """Write a mapping in the block YAML style of the WinGet catalog."""
    pad = " " * indent
    lines = []
    for key, value in mapping.items():
        if isinstance(value, dict):
            lines += [f"{pad}{key}:"] + yaml_lines(value, indent + 2)
        elif isinstance(value, list):
            lines.append(f"{pad}{key}:")
            for item in value:
                if isinstance(item, dict):
                    nested = yaml_lines(item, indent + 2)
                    lines += [f"{pad}- {nested[0].lstrip()}"] + nested[1:]
                else:
                    lines.append(f"{pad}- {yaml_scalar(item)}")
        else:
            lines.append(f"{pad}{key}: {yaml_scalar(value)}")
    return lines


def yaml_scalar(value):
    """Quote a string only where plain YAML would change its meaning."""
    text = str(value)
    plain = re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9 ._/:+()-]*", text) and ": " not in text and not text.endswith(":")
    special = text.lower() in {"true", "false", "yes", "no", "on", "off", "null", "~"}
    number = re.fullmatch(r"[-+]?(\d[\d_]*)?(\.\d*)?([eE][-+]?\d+)?", text)
    if plain and not special and not number:
        return text
    return "'" + text.replace("'", "''") + "'"


def write(path, content):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8", newline="\n")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    archive = commands.add_parser("pack")
    archive.add_argument("--target", choices=TARGETS, required=True)
    archive.add_argument("--binary", type=Path, required=True)
    archive.add_argument("--out", type=Path, default=Path("dist"))
    channels = commands.add_parser("metadata")
    channels.add_argument("--artifacts", type=Path, default=Path("dist"))
    channels.add_argument("--out", type=Path, default=Path("dist/packages"))
    args = parser.parse_args()
    if args.command == "pack":
        pack(args.target, args.binary, args.out)
    else:
        metadata(args.artifacts, args.out)
