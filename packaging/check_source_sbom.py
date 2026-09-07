"""Require the source SPDX SBOM to contain every Cargo.lock package/version.

This is a source inventory, including development and platform-specific packages,
not an assertion that every package is linked into each native executable.
"""

import argparse
import json
from pathlib import Path
import tomllib


def validate(lock, sbom):
    if not sbom.get("spdxVersion", "").startswith("SPDX-2."):
        raise ValueError("expected SPDX 2.x JSON")
    creators = sbom.get("creationInfo", {}).get("creators", [])
    if not any(creator.startswith("Tool: syft-") for creator in creators):
        raise ValueError("missing Syft scanner identity/version")
    expected = {(package["name"], package["version"]) for package in lock["package"]}
    actual = {(package["name"], package.get("versionInfo")) for package in sbom["packages"]}
    missing = sorted(expected - actual)
    if missing:
        raise ValueError(f"source SBOM omits locked packages: {missing}")
    return len(expected)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("lockfile", type=Path)
    parser.add_argument("sbom", type=Path)
    args = parser.parse_args()
    lock = tomllib.loads(args.lockfile.read_text(encoding="utf-8"))
    sbom = json.loads(args.sbom.read_text(encoding="utf-8"))
    count = validate(lock, sbom)
    print(f"Source SBOM covers all {count} locked package/version pairs (including dev/target dependencies)")


if __name__ == "__main__":
    main()
