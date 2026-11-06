import base64
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch
import xml.etree.ElementTree as ET

from cryptography.exceptions import InvalidSignature
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
import release


class ReleaseTests(unittest.TestCase):
    def setUp(self):
        scratch = release.ROOT.joinpath("target/release-tests")
        scratch.mkdir(parents=True, exist_ok=True)
        self.directory = Path(tempfile.mkdtemp(dir=scratch))
        self.key = Ed25519PrivateKey.from_private_bytes(bytes(range(32)))
        self.encoded = base64.b64encode(self.key.private_bytes_raw()).decode()
        self.public = self.directory.joinpath("public-key.txt")
        self.public.write_text(base64.b64encode(self.key.public_key().public_bytes_raw()).decode())
        self.payload = self.directory.joinpath("Sevi-1.0.30-x64-Setup.exe")
        self.payload.write_bytes(b"MZ-test-payload-for-signature-contract")

    def test_feed_signature_covers_every_installer_byte(self):
        with patch.object(release, "PUBLIC", self.public), patch.object(release, "version", return_value="1.0.30"):
            xml = release.appcast("v1.0.30", self.payload, self.encoded)
            self.assertIn(b"<item>", xml)
            self.assertIn(b"<sparkle:shortVersionString>1.0.30</sparkle:shortVersionString>", xml)
            self.assertIn(b'sevi:arch="x64"', xml)
            enclosure = ET.fromstring(xml).find("channel/item/enclosure")
            self.assertEqual(enclosure.attrib["url"], "https://github.com/tuanxtm/sevi/releases/download/v1.0.30/" + self.payload.name)
            self.assertEqual(int(enclosure.attrib["length"]), self.payload.stat().st_size)
            signature = base64.b64decode(enclosure.attrib["{" + release.SPARKLE + "}edSignature"])
            self.key.public_key().verify(signature, self.payload.read_bytes())
            with self.assertRaises(InvalidSignature):
                self.key.public_key().verify(signature, self.payload.read_bytes() + b"changed")

    def test_wrong_key_and_mismatched_or_unsafe_tags_fail(self):
        other = base64.b64encode(Ed25519PrivateKey.generate().private_bytes_raw()).decode()
        with patch.object(release, "PUBLIC", self.public), patch.object(release, "version", return_value="1.0.30"):
            with self.assertRaises(ValueError): release.private_key(other)
            for tag in ["v1.0.31", "main", "v1.0.30;echo bad", "v1.0.30-beta"]:
                with self.assertRaises(ValueError): release.check_tag(tag)

    def test_published_assets_cannot_be_replaced(self):
        result = subprocess.CompletedProcess([], 0, json.dumps({"isDraft": False}), "")
        with patch.object(release, "check_tag"), patch.object(release.subprocess, "run", return_value=result) as run:
            with self.assertRaisesRegex(ValueError, "already published"):
                release.publish("v1.0.30")
            self.assertEqual(run.call_count, 1)

    def test_draft_assets_are_uploaded_before_release_becomes_visible(self):
        result = subprocess.CompletedProcess([], 0, json.dumps({"isDraft": True}), "")
        with patch.object(release, "check_tag"), patch.object(release.subprocess, "run", return_value=result) as run:
            release.publish("v1.0.30")
            commands = [call.args[0] for call in run.call_args_list]
            self.assertEqual([command[2] for command in commands], ["view", "upload", "edit"])
            self.assertIn("--draft=false", commands[-1])


if __name__ == "__main__": unittest.main()
