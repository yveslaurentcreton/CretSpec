import json
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
                            "LICENSE": b"MIT", "README.md": b"Readme"}
                data = package.archive_bytes(target, contents)
                self.assertEqual(data, package.archive_bytes(target, contents))
                (root / package.archive_name("0.4.0", target)).write_bytes(data)
            package.metadata(root, root / "packages", "0.4.0")
            manifest = next((root / "packages/winget").rglob("*.installer.yaml"))
            installer = json.loads(manifest.read_text())["Installers"][0]
            expected = package.inspect_archive(root / package.archive_name("0.4.0", package.TARGETS[0]), package.TARGETS[0])
            self.assertEqual(installer["InstallerSha256"], expected.upper())
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


if __name__ == "__main__":
    unittest.main()
