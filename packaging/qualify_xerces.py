"""Explicit, non-skipping independent-backend qualification using generic fixtures."""
import argparse
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

import verify_schemas as common
import verify_schemas_xerces as xerces
from test_verify_schemas import TYPES, WRAPPER, VALID, EXPECTED


class Qualification(unittest.TestCase):
    tool_root: Path
    java: str

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="oxvif-xerces-qualification-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.manifest = {"format": 1, "sources": []}
        for url, data in [("https://example.invalid/types/shared.xsd", TYPES),
                          ("https://example.invalid/service/root.xsd", WRAPPER)]:
            self.manifest["sources"].append({"url": url, "sha256": hashlib.sha256(data).hexdigest()})
            path = common.source_path(self.root, url)
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
        self.catalogue = common.Catalogue(self.root, self.manifest)
        self.corpus = self.root / "corpus"
        self.corpus.mkdir()
        self.instance = self.corpus / "one.xml"
        self.instance.write_bytes(VALID)
        (self.corpus / "cases.json").write_text(json.dumps({"format": 1, "cases": [{"file": "one.xml", "root": EXPECTED}]}), encoding="utf-8")

    def check(self):
        return xerces.run(self.catalogue, self.tool_root, self.java, self.corpus)

    def test_valid_relative_import_and_instance(self):
        self.assertEqual(self.check(), "PASS: Xerces strict XSD 1.1 instances=1; not semantic conformance")

    def test_payload_anchor_uses_inherited_namespace_scope(self):
        index = self.corpus / 'cases.json'
        value = {'format': 1, 'cases': [{'file': 'one.xml', 'root': EXPECTED, 'payload_path': ['{urn:verify}Packet']}]}
        index.write_text(json.dumps(value), encoding='utf-8')
        self.assertEqual(self.check(), "PASS: Xerces strict XSD 1.1 instances=1; not semantic conformance")
        value['cases'][0]['payload_path'] = ['{urn:verify}Other']
        index.write_text(json.dumps(value), encoding='utf-8')
        with self.assertRaises(common.VerificationError) as error:
            self.check()
        self.assertEqual(str(error.exception), 'Xerces compilation/validation failed: IllegalArgumentException')

    def test_lax_wrapper_does_not_hide_unknown_or_multiple_payloads(self):
        url, _, path = self.catalogue.entries[1]
        modified = WRAPPER.replace(b'<xs:element ref="v:Packet"/>', b'<xs:any processContents="lax" minOccurs="0" maxOccurs="unbounded"/>')
        path.write_bytes(modified)
        self.manifest['sources'][1]['sha256'] = hashlib.sha256(modified).hexdigest()
        self.catalogue = common.Catalogue(self.root, self.manifest)
        self.instance.write_bytes(b'<w:Wrapper xmlns:w="urn:wrapper"><u:Unknown xmlns:u="urn:unlisted"/></w:Wrapper>')
        self.assertEqual(self.check(), "PASS: Xerces strict XSD 1.1 instances=1; not semantic conformance")
        index = self.corpus / 'cases.json'
        value = {'format': 1, 'cases': [{'file': 'one.xml', 'root': EXPECTED, 'payload_path': ['{urn:unlisted}Unknown']}]}
        index.write_text(json.dumps(value), encoding='utf-8')
        with self.assertRaises(common.VerificationError) as error:
            self.check()
        self.assertEqual(str(error.exception), 'Xerces compilation/validation failed: SAXParseException:cvc-elt.1.a')
        self.instance.write_bytes(VALID.replace(b'</w:Wrapper>', b'<u:Other xmlns:u="urn:unlisted"/></w:Wrapper>'))
        value['cases'][0]['payload_path'] = ['{urn:verify}Packet']
        index.write_text(json.dumps(value), encoding='utf-8')
        with self.assertRaises(common.VerificationError) as error:
            self.check()
        self.assertEqual(str(error.exception), 'Xerces compilation/validation failed: IllegalArgumentException')

    def test_invalid_value_qname_cardinality_order_attribute_namespace_and_root(self):
        mutations = [VALID.replace(b">9<", b">-2<"), VALID.replace(b"c:Known", b"missing:Known"),
                     VALID.replace(b'<v:Count>9</v:Count>', b''),
                     VALID.replace(b'<v:Count>9</v:Count>', b'<v:Count>9</v:Count><v:Count>8</v:Count>'),
                     VALID.replace(b'<v:Count>9</v:Count><v:Code>c:Known</v:Code>', b'<v:Code>c:Known</v:Code><v:Count>9</v:Count>'),
                     VALID.replace(b' id="27"', b''),
                     VALID.replace(b'<v:Count>', b'<Count>').replace(b'</v:Count>', b'</Count>'),
                     VALID.replace(b'w:Wrapper', b'w:Other')]
        categories = ["SAXParseException:cvc-minInclusive-valid", "SAXParseException:UndeclaredPrefix",
                      "SAXParseException:cvc-complex-type.2.4.a", "SAXParseException:cvc-complex-type.2.4.a",
                      "SAXParseException:cvc-complex-type.2.4.a", "SAXParseException:cvc-complex-type.4",
                      "SAXParseException:cvc-complex-type.2.4.a", "IllegalArgumentException"]
        for index, data in enumerate(mutations):
            with self.subTest(case=index):
                self.instance.write_bytes(data)
                with self.assertRaises(common.VerificationError) as error:
                    self.check()
                self.assertEqual(str(error.exception), "Xerces compilation/validation failed: " + categories[index])

    def test_dtd_and_unlisted_import_are_refused(self):
        self.instance.write_bytes(('<!DOCTYPE Wrapper>' + VALID.decode()).encode('utf-16'))
        with self.assertRaisesRegex(common.VerificationError, "DTD is forbidden"):
            self.check()
        self.instance.write_bytes(VALID)
        url, _, path = self.catalogue.entries[1]
        modified = WRAPPER.replace(b'../types/shared.xsd', b'https://unlisted.invalid/schema.xsd')
        path.write_bytes(modified)
        self.manifest['sources'][1]['sha256'] = hashlib.sha256(modified).hexdigest()
        self.catalogue = common.Catalogue(self.root, self.manifest)
        with self.assertRaisesRegex(common.VerificationError, "unlisted schema resource"):
            self.check()

    def test_bad_schema_is_rejected_by_independent_compiler(self):
        url, _, path = self.catalogue.entries[0]
        modified = TYPES.replace(b'type="xs:positiveInteger"', b'type="xs:DoesNotExist"')
        path.write_bytes(modified)
        self.manifest['sources'][0]['sha256'] = hashlib.sha256(modified).hexdigest()
        self.catalogue = common.Catalogue(self.root, self.manifest)
        with self.assertRaises(common.VerificationError) as error:
            self.check()
        self.assertEqual(str(error.exception), "Xerces compilation/validation failed: SAXParseException:src-resolve.4.2")

    def test_location_hint_cannot_replace_pinned_schema(self):
        self.instance.write_bytes(VALID.replace(b'<w:Wrapper ', b'<w:Wrapper xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:schemaLocation="urn:wrapper https://unlisted.invalid/schema.xsd" '))
        self.assertEqual(self.check(), "PASS: Xerces strict XSD 1.1 instances=1; not semantic conformance")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tool-root", type=Path, required=True)
    parser.add_argument("--java", default="java")
    args = parser.parse_args()
    Qualification.tool_root, Qualification.java = args.tool_root, args.java
    # Missing dependencies are errors, never conditional skips.
    xerces.verify_tool(args.tool_root)
    result = unittest.TextTestRunner(verbosity=2).run(unittest.defaultTestLoader.loadTestsFromTestCase(Qualification))
    return 0 if result.wasSuccessful() else 1


if __name__ == "__main__":
    raise SystemExit(main())
