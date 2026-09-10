"""Schema-free extraction and launch guards; Java qualification is separate."""
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from xml.dom import minidom
import xml.etree.ElementTree as ET

import verify_schemas as common
import verify_schemas_xerces as xerces


class XercesToolingTests(unittest.TestCase):
    def test_inline_extraction_preserves_owner_and_descendant_prefixes(self):
        xml = '<root xmlns:q="urn:outer" xmlns:v="urn:v"><owner xmlns:q="urn:owner"><schema><q:One value="q:Kind"/><child xmlns:q="urn:child"><q:Two/></child><q:Three/></schema></owner></root>'
        document = minidom.parseString(xml)
        element = document.getElementsByTagName("schema")[0]
        extracted = xerces.inline_schema(element)
        root = ET.fromstring(extracted)
        self.assertEqual([node.tag for node in root], ["{urn:owner}One", "child", "{urn:owner}Three"])
        self.assertEqual(root[0].attrib["value"], "q:Kind")
        self.assertEqual(root[1][0].tag, "{urn:child}Two")
        self.assertIn(b'xmlns:q="urn:owner"', extracted)
        self.assertIn(b'xmlns:v="urn:v"', extracted)
        self.assertFalse(element.hasAttribute("xmlns:q"), "original DOM must not be modified")

    def test_metadata_rejects_control_character_injection(self):
        self.assertEqual(xerces.row("urn:root", "plain.xml"), "urn:root\tplain.xml")
        for value in ["a\tb", "a\nb", "a\rb"]:
            with self.assertRaisesRegex(common.VerificationError, "control character"):
                xerces.row(value, "plain.xml")

    def test_missing_and_tampered_tool_dependencies_fail(self):
        with tempfile.TemporaryDirectory(prefix="oxvif-xerces-control-") as temporary:
            root = Path(temporary)
            with self.assertRaisesRegex(common.VerificationError, "missing or mismatched"):
                xerces.verify_tool(root)
            for name in xerces.tool_manifest()["jars"]:
                (root / name).write_bytes(b"untrusted")
            with self.assertRaisesRegex(common.VerificationError, "missing or mismatched"):
                xerces.verify_tool(root)
            with patch.object(xerces, "build_opener") as network:
                with self.assertRaisesRegex(common.VerificationError, "missing or mismatched"):
                    xerces.fetch_tool(root)
                network.assert_not_called()


if __name__ == "__main__":
    unittest.main()
