import copy
import unittest

from check_source_sbom import validate


class SourceSbomTests(unittest.TestCase):
    def setUp(self):
        self.lock = {"package": [{"name": "oxvif-cli", "version": "0.16.0"},
                                  {"name": "sha2", "version": "0.10.9"},
                                  {"name": "sha2", "version": "0.11.0"}]}
        self.sbom = {"spdxVersion": "SPDX-2.3",
                     "creationInfo": {"creators": ["Tool: syft-1.51.1"]},
                     "packages": [{"name": p["name"], "versionInfo": p["version"]}
                                  for p in self.lock["package"]]}

    def test_complete_inventory_accepts_multiple_versions(self):
        self.assertEqual(validate(self.lock, self.sbom), 3)

    def test_missing_or_wrong_version_fails(self):
        for package_index in range(3):
            for replacement in [None, "UNKNOWN"]:
                sbom = copy.deepcopy(self.sbom)
                if replacement is None:
                    sbom["packages"].pop(package_index)
                else:
                    sbom["packages"][package_index]["versionInfo"] = replacement
                with self.assertRaises(ValueError):
                    validate(self.lock, sbom)

    def test_empty_binary_only_inventory_fails(self):
        self.sbom["packages"] = [{"name": "release", "primaryPackagePurpose": "FILE"}]
        with self.assertRaises(ValueError):
            validate(self.lock, self.sbom)

    def test_missing_scanner_or_wrong_format_fails(self):
        for key, value in [("creationInfo", {}), ("spdxVersion", "invalid")]:
            sbom = copy.deepcopy(self.sbom)
            sbom[key] = value
            with self.assertRaises(ValueError):
                validate(self.lock, sbom)


if __name__ == "__main__":
    unittest.main()
