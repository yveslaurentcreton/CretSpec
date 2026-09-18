"""Catalog preparation must reject substituted release downloads."""

import importlib.util
import io
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import package

module = importlib.util.spec_from_file_location("channels", Path(__file__).with_name("prepare-channels.py"))
channels = importlib.util.module_from_spec(module)
module.loader.exec_module(channels)


class ChannelPreparation(unittest.TestCase):
    def test_changed_download_cannot_generate_catalog_metadata(self):
        name = package.archive_name("0.4.0", package.TARGETS[0])
        checksums = f"{'0' * 64}  {name}\n".encode()
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            with patch.object(channels.urllib.request, "urlopen", side_effect=[io.BytesIO(checksums), io.BytesIO(b"substituted archive")]):
                with self.assertRaisesRegex(ValueError, "checksum mismatch"):
                    channels.prepare("0.4.0", output)
            self.assertFalse((output / "packages").exists())
            self.assertEqual(list((output / "archives").iterdir()), [])

    def test_invalid_release_is_rejected_before_network_or_filesystem_changes(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "output"
            with patch.object(channels.urllib.request, "urlopen") as download:
                with self.assertRaisesRegex(ValueError, "stable release"):
                    channels.prepare("../latest", output)
                download.assert_not_called()
            self.assertFalse(output.exists())


if __name__ == "__main__":
    unittest.main()
