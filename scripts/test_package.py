from pathlib import Path
import tempfile
import unittest

import package


class Packages(unittest.TestCase):
    def test_metadata_uses_exact_archives_and_one_version(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for target in package.TARGETS:
                contents = {"cspec.exe" if "windows" in target else "cspec": target.encode(),
                            "LICENSE": b"MIT", "README.md": b"Readme",
                            "THIRD-PARTY.txt": b"Notices", "RUST-LICENSES.html": b"Rust notices"}
                data = package.archive_bytes(target, contents)
                self.assertEqual(data, package.archive_bytes(target, contents))
                (root / package.archive_name("0.4.0", target)).write_bytes(data)
            package.metadata(root, root / "packages", "0.4.0")
            manifest = next((root / "packages/winget").rglob("*.installer.yaml")).read_text()
            expected = package.inspect_archive(root / package.archive_name("0.4.0", package.TARGETS[0]), package.TARGETS[0])
            self.assertIn(f"  InstallerSha256: {expected.upper()}\n", manifest)
            self.assertIn("NestedInstallerFiles:\n- RelativeFilePath: cspec.exe\n  PortableCommandAlias: cspec\n", manifest)
            self.assertIn("UpgradeBehavior: uninstallPrevious\n", manifest)
            self.assertNotIn("Commands:", manifest)
            for path in (root / "packages/winget").rglob("*.yaml"):
                lines = path.read_text().splitlines()
                self.assertTrue(lines[0].startswith("# yaml-language-server: $schema="))
                self.assertIn(f".{package.WINGET_MANIFEST_VERSION}.schema.json", lines[0])
                self.assertEqual(lines[-1], f"ManifestVersion: {package.WINGET_MANIFEST_VERSION}")
                self.assertFalse(any(character in path.read_text() for character in "{}[]\""))
            self.assertEqual(len((root / "SHA256SUMS").read_text().splitlines()), 4)
            self.assertIn("pkgver=0.4.0", (root / "packages/aur/PKGBUILD").read_text())

    def test_incomplete_matrix_and_invalid_versions_emit_nothing(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with self.assertRaises(FileNotFoundError):
                package.metadata(root, root / "packages", "0.4.0")
            with self.assertRaises(ValueError):
                package.metadata(root, root / "packages", "0.4.0;command")
            self.assertFalse((root / "packages").exists())

    def test_unsafe_archive_members_are_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "unsafe.tar.gz"
            path.write_bytes(package.archive_bytes(package.TARGETS[1], {"../cspec": b"bad"}))
            with self.assertRaises(ValueError):
                package.inspect_archive(path, package.TARGETS[1])

    def test_windows_runtime_imports_are_found(self):
        executable = portable_executable(["KERNEL32.dll", "VCRUNTIME140.dll", "api-ms-win-crt-heap-l1-1-0.dll"])
        imports = package.pe_imports(executable)
        self.assertEqual(imports, ["KERNEL32.dll", "VCRUNTIME140.dll", "api-ms-win-crt-heap-l1-1-0.dll"])
        self.assertEqual([name for name in imports if package.RUNTIME_DLL.match(name)],
                         ["VCRUNTIME140.dll", "api-ms-win-crt-heap-l1-1-0.dll"])
        self.assertEqual(package.pe_imports(portable_executable(["KERNEL32.dll", "ntdll.dll"])),
                         ["KERNEL32.dll", "ntdll.dll"])
        with self.assertRaises(ValueError):
            package.pe_imports(b"\x7fELF" + bytes(512))

    def test_yaml_scalars_stay_strings(self):
        self.assertEqual(package.yaml_scalar("0.8.1"), "0.8.1")
        self.assertEqual(package.yaml_scalar("https://example.com/a-b_c.zip"), "https://example.com/a-b_c.zip")
        self.assertEqual(package.yaml_scalar("1.12"), "'1.12'")
        self.assertEqual(package.yaml_scalar("12E4"), "'12E4'")
        self.assertEqual(package.yaml_scalar("yes"), "'yes'")
        self.assertEqual(package.yaml_scalar("a: b"), "'a: b'")
        self.assertEqual(package.yaml_scalar("it's"), "'it''s'")
        self.assertEqual(package.yaml_lines({"A": [{"B": "c", "D": {"E": ["f"]}}]}),
                         ["A:", "- B: c", "  D:", "    E:", "    - f"])


def portable_executable(dlls):
    """Build a minimal PE32+ image with one section that holds the import table."""
    section_rva, raw_offset = 0x1000, 0x200
    names = b""
    name_offsets = []
    table_size = 20 * (len(dlls) + 1)
    for dll in dlls:
        name_offsets.append(table_size + len(names))
        names += dll.encode("ascii") + b"\0"
    table = b"".join(bytes(12) + (section_rva + offset).to_bytes(4, "little") + bytes(4) for offset in name_offsets)
    raw = table + bytes(20) + names
    image = bytearray(raw_offset + len(raw))
    image[0:2] = b"MZ"
    image[0x3C:0x40] = (0x40).to_bytes(4, "little")
    image[0x40:0x44] = b"PE\0\0"
    coff = 0x44
    image[coff:coff + 2] = (0x8664).to_bytes(2, "little")
    image[coff + 2:coff + 4] = (1).to_bytes(2, "little")
    image[coff + 16:coff + 18] = (240).to_bytes(2, "little")
    optional = coff + 20
    image[optional:optional + 2] = (0x20B).to_bytes(2, "little")
    image[optional + 120:optional + 124] = section_rva.to_bytes(4, "little")
    image[optional + 124:optional + 128] = len(raw).to_bytes(4, "little")
    section = optional + 240
    image[section:section + 6] = b".idata"
    image[section + 8:section + 12] = len(raw).to_bytes(4, "little")
    image[section + 12:section + 16] = section_rva.to_bytes(4, "little")
    image[section + 16:section + 20] = len(raw).to_bytes(4, "little")
    image[section + 20:section + 24] = raw_offset.to_bytes(4, "little")
    image[raw_offset:] = raw
    return bytes(image)


if __name__ == "__main__":
    unittest.main()
