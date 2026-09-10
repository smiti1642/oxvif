"""Independent generic controls; no official schemas or derived data in the repo."""

from contextlib import redirect_stderr
import hashlib
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from urllib.request import Request
import warnings

import verify_schemas as v


# Entirely project-authored vocabulary: these are validator sensitivity cases,
# not a substitute for external ONVIF request/response acceptance.
TYPES = b'''<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema" targetNamespace="urn:verify" xmlns:v="urn:verify" elementFormDefault="qualified">
<xs:element name="Packet"><xs:complexType><xs:sequence>
<xs:element name="Count" type="xs:positiveInteger"/>
<xs:element name="Code" type="xs:QName"/>
</xs:sequence><xs:attribute name="id" type="xs:integer" use="required"/></xs:complexType></xs:element>
</xs:schema>'''
WRAPPER = b'''<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema" targetNamespace="urn:wrapper" xmlns:v="urn:verify" elementFormDefault="qualified">
<xs:import namespace="urn:verify" schemaLocation="../types/shared.xsd"/>
<xs:element name="Wrapper"><xs:complexType><xs:sequence>
<xs:element ref="v:Packet"/>
</xs:sequence></xs:complexType></xs:element></xs:schema>'''
VALID = b'<w:Wrapper xmlns:w="urn:wrapper" xmlns:v="urn:verify" xmlns:c="urn:codes"><v:Packet id="27"><v:Count>9</v:Count><v:Code>c:Known</v:Code></v:Packet></w:Wrapper>'
EXPECTED = "{urn:wrapper}Wrapper"


class CatalogueTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="oxvif-verifier-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.manifest = {"format": 1, "sources": []}
        for url, data in [("https://example.invalid/types/shared.xsd", TYPES),
                          ("https://example.invalid/service/root.xsd", WRAPPER)]:
            self.manifest["sources"].append({"url": url, "sha256": hashlib.sha256(data).hexdigest()})
            path = v.source_path(self.root, url)
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
        self.catalogue = v.Catalogue(self.root, self.manifest)

    def test_manifest_rejects_unpinned_duplicate_and_escaping_sources(self):
        for url in ["http://example.invalid/x.xsd", "https://user:password@example.invalid/x.xsd",
                    "https://example.invalid/../x.xsd", "https://example.invalid/%2e%2e/x.xsd",
                    "https://example.invalid/a\\b.xsd", "https://example.invalid/a.xsd?q=x"]:
            with self.subTest(url=url), self.assertRaisesRegex(v.VerificationError, "invalid source URL"):
                v.source_path(self.root, url)
        self.manifest["sources"].append(self.manifest["sources"][0])
        with self.assertRaisesRegex(v.VerificationError, "duplicate"):
            v.Catalogue(self.root, self.manifest)

    def test_checkout_and_ancestor_directories_are_refused(self):
        for path in [v.REPOSITORY, v.REPOSITORY / "target", v.REPOSITORY.parent]:
            with self.assertRaisesRegex(v.VerificationError, "outside"):
                v.external_root(path)
        self.assertEqual(v.external_root(self.root), self.root.resolve())

    def test_hash_and_missing_file_fail_before_compilation(self):
        self.catalogue.verify_files()
        path = self.catalogue.entries[0][2]
        path.write_bytes(TYPES + b" ")
        with self.assertRaisesRegex(v.VerificationError, "digest mismatch"):
            self.catalogue.verify_files()
        path.unlink()
        with self.assertRaisesRegex(v.VerificationError, "missing"):
            self.catalogue.verify_files()

    def test_offline_resolver_rejects_unlisted_and_final_path_escape(self):
        url, _, path = self.catalogue.entries[0]
        self.assertEqual(self.catalogue.resolve(url), path.as_uri())
        self.assertEqual(self.catalogue.resolve("http:" + url[6:]), path.as_uri())
        with self.assertRaisesRegex(v.VerificationError, "unlisted schema"):
            self.catalogue.resolve("https://unexpected.invalid/secret.xsd")
        with self.assertRaisesRegex(v.VerificationError, "unlisted absolute"):
            v.PinnedFiles(self.catalogue).file_open(Request((self.root / "other.xsd").as_uri()))

    def test_source_closure_and_instances_validate_without_network(self):
        with patch("socket.create_connection", side_effect=AssertionError("network attempted")) as network:
            schema, roots, edges = v.compile_catalogue(self.catalogue)
            v.validate_instance(schema, VALID, EXPECTED)
            network.assert_not_called()
        self.assertEqual((roots, edges), (1, 1))

    def test_wsdl_inline_schema_retains_inherited_qname_bindings(self):
        inline = WRAPPER.replace(b' xmlns:xs="http://www.w3.org/2001/XMLSchema"', b'').replace(b' xmlns:v="urn:verify"', b'')
        data = b'<w:definitions xmlns:w="http://schemas.xmlsoap.org/wsdl/" xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:v="urn:verify"><w:types>' + inline + b'</w:types></w:definitions>'
        path = self.catalogue.entries[1][2]
        path.write_bytes(data)
        self.manifest["sources"][1]["sha256"] = hashlib.sha256(data).hexdigest()
        schema, roots, edges = v.compile_catalogue(v.Catalogue(self.root, self.manifest))
        v.validate_instance(schema, VALID, EXPECTED)
        self.assertEqual((roots, edges), (1, 1))

    def test_wrong_dependency_version_fails_before_loading_schema(self):
        with patch("importlib.metadata.version", return_value="0.0.0"):
            with self.assertRaisesRegex(v.VerificationError, "dependency version mismatch"):
                v.compile_catalogue(self.catalogue)

    def test_xsd_rejects_value_qname_shape_and_namespace_mutations(self):
        schema, _, _ = v.compile_catalogue(self.catalogue)
        mutations = [VALID.replace(b">9<", b">-2<"), VALID.replace(b"c:Known", b"missing:Known"),
                     VALID.replace(b'<v:Count>9</v:Count>', b''),
                     VALID.replace(b'<v:Count>9</v:Count>', b'<v:Count>9</v:Count><v:Count>8</v:Count>'),
                     VALID.replace(b'<v:Count>9</v:Count><v:Code>c:Known</v:Code>', b'<v:Code>c:Known</v:Code><v:Count>9</v:Count>'),
                     VALID.replace(b' id="27"', b''), VALID.replace(b'<v:Count>', b'<Count>').replace(b'</v:Count>', b'</Count>')]
        expected_paths = ["/w:Wrapper/v:Packet/v:Count", "/w:Wrapper/v:Packet/v:Code"] + ["/w:Wrapper/v:Packet"] * 5
        for data, path in zip(mutations, expected_paths, strict=True):
            with self.subTest(data=data):
                with self.assertRaises(v.runtime().XMLSchemaValidationError) as error:
                    v.validate_instance(schema, data, EXPECTED)
                self.assertEqual(error.exception.path, path)

    def test_root_and_schema_location_hints_cannot_bypass_validation(self):
        schema, _, _ = v.compile_catalogue(self.catalogue)
        with self.assertRaisesRegex(v.VerificationError, "root differs"):
            v.validate_instance(schema, VALID, "{urn:wrapper}Other")
        with self.assertRaisesRegex(v.VerificationError, "no schema declaration"):
            v.validate_instance(schema, b'<Unknown/>', "Unknown")
        hinted = VALID.replace(b'<w:Wrapper ', b'<w:Wrapper xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:schemaLocation="urn:wrapper https://unlisted.invalid/x.xsd" ')
        with patch("socket.create_connection", side_effect=AssertionError("network attempted")) as network:
            v.validate_instance(schema, hinted, EXPECTED)
            network.assert_not_called()

    def test_unlisted_import_and_dtd_fail_closed(self):
        for data in [WRAPPER.replace(b'../types/shared.xsd', b'https://unlisted.invalid/types.xsd'),
                     b'<!DOCTYPE x [<!ENTITY e "value">]>' + WRAPPER]:
            url, _, path = self.catalogue.entries[1]
            path.write_bytes(data)
            self.manifest["sources"][1]["sha256"] = hashlib.sha256(data).hexdigest()
            catalogue = v.Catalogue(self.root, self.manifest)
            with self.assertRaises(v.VerificationError):
                v.compile_catalogue(catalogue)

    def test_dtd_rejection_handles_utf16_and_plain_doctypes(self):
        for encoding in ["utf-8", "utf-16"]:
            for text in ['<!DOCTYPE Wrapper>' + VALID.decode(), '<!DOCTYPE Wrapper [<!ENTITY value "x">]>' + VALID.decode()]:
                with self.assertRaisesRegex(v.VerificationError, "DTD is forbidden"):
                    v.check_xml_bytes(text.encode(encoding))
        v.check_xml_bytes(VALID)

    def test_fetch_reuses_matching_files_and_never_overwrites_mismatch(self):
        with patch.object(v, "build_opener") as opener:
            self.catalogue.fetch()
            opener.return_value.open.assert_not_called()
            path = self.catalogue.entries[0][2]
            path.write_bytes(b"existing-local-data")
            with self.assertRaisesRegex(v.VerificationError, "refusing to overwrite"):
                self.catalogue.fetch()
            self.assertEqual(path.read_bytes(), b"existing-local-data")
            opener.return_value.open.assert_not_called()

    def test_corpus_requires_complete_explicit_nonempty_cases(self):
        schema, _, _ = v.compile_catalogue(self.catalogue)
        corpus = self.root / "corpus"
        corpus.mkdir()
        (corpus / "one.xml").write_bytes(VALID)
        index = corpus / "cases.json"
        valid = {"file": "one.xml", "root": EXPECTED}
        index.write_text(json.dumps({"format": 1, "cases": [valid]}), encoding="utf-8")
        self.assertEqual(v.validate_corpus(schema, corpus), 1)
        for cases in [[], [valid, valid], [{"file": "../one.xml", "root": EXPECTED}], [{"file": "absent.xml", "root": EXPECTED}]]:
            index.write_text(json.dumps({"format": 1, "cases": cases}), encoding="utf-8")
            with self.assertRaises(v.VerificationError):
                v.validate_corpus(schema, corpus)

    def test_cli_sanitizes_validator_exceptions_and_fails(self):
        manifest = self.root / "manifest.json"
        manifest.write_text(json.dumps(self.manifest), encoding="utf-8")
        output = io.StringIO()
        with patch.object(v, "compile_catalogue", side_effect=ValueError("schema excerpt or credential")), redirect_stderr(output):
            result = v.main(["compile", "--root", str(self.root), "--manifest", str(manifest)])
        self.assertEqual(result, 1)
        self.assertEqual(output.getvalue(), "FAIL: ValueError\n")

    def test_compile_warnings_fail_instead_of_being_suppressed(self):
        class WarningSchema:
            @staticmethod
            def XMLSchema11(*args, **kwargs):
                warnings.warn("generic qualification warning", UserWarning)
        with patch.object(v, "runtime", return_value=WarningSchema), patch.object(v, "resources", return_value=([object()], 0)):
            with self.assertRaisesRegex(UserWarning, "qualification warning"):
                v.compile_catalogue(self.catalogue)


if __name__ == "__main__":
    unittest.main()
