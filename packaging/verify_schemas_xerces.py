"""Independent Xerces XSD 1.1 backend. All downloaded/derived data stays external."""
from __future__ import annotations

import argparse
import hashlib
import io
import json
import os
import re
from pathlib import Path
import subprocess
import sys
import tempfile
from urllib.request import build_opener
from xml.dom import minidom
import zipfile

import verify_schemas as common

TOOL_MANIFEST = Path(__file__).with_name("xerces-validator.json")
JAVA_SOURCE = Path(__file__).with_name("SchemaVerifier.java")
MAX_TOOL_BYTES = 64 * 1024 * 1024


def tool_manifest():
    return json.loads(TOOL_MANIFEST.read_text(encoding="utf-8"))


def verify_tool(root: Path) -> list[Path]:
    root = common.external_root(root)
    paths = []
    for name, digest in tool_manifest()["jars"].items():
        path = root / name
        if (not path.resolve().is_relative_to(root) or not path.is_file()
                or path.stat().st_size > MAX_TOOL_BYTES
                or hashlib.sha256(path.read_bytes()).hexdigest() != digest):
            raise common.VerificationError("missing or mismatched Xerces dependency")
        paths.append(path)
    return paths


def fetch_tool(root: Path) -> None:
    root = common.external_root(root)
    manifest = tool_manifest()
    if all((root / name).is_file() for name in manifest["jars"]):
        verify_tool(root)
        return
    opener = build_opener(common.HttpsRedirects())
    with opener.open(manifest["archive_url"], timeout=30) as response:
        data = response.read(MAX_TOOL_BYTES + 1)
    if len(data) > MAX_TOOL_BYTES or hashlib.sha512(data).hexdigest() != manifest["archive_sha512"]:
        raise common.VerificationError("Xerces archive digest or size mismatch")
    with zipfile.ZipFile(io.BytesIO(data)) as archive:
        for name, digest in manifest["jars"].items():
            member = archive.getinfo(manifest["archive_prefix"] + name)
            if member.file_size > MAX_TOOL_BYTES:
                raise common.VerificationError("oversized Xerces member")
            contents = archive.read(member)
            if hashlib.sha256(contents).hexdigest() != digest:
                raise common.VerificationError("Xerces member digest mismatch")
            path = root / name
            if path.exists():
                if hashlib.sha256(path.read_bytes()).hexdigest() != digest:
                    raise common.VerificationError("refusing to overwrite mismatched Xerces member")
                continue
            root.mkdir(parents=True, exist_ok=True)
            with path.open("xb") as output:
                output.write(contents)
    verify_tool(root)


def row(*fields: str) -> str:
    if any(any(char in field for char in "\t\r\n") for field in fields):
        raise common.VerificationError("control character in verifier metadata")
    return "\t".join(fields)


def inline_schema(element) -> bytes:
    """Copy only ancestor namespace context; retain all descendant declarations."""
    ancestors = []
    current = element
    while current.nodeType == current.ELEMENT_NODE:
        ancestors.append(current)
        current = current.parentNode
    namespaces = {}
    for ancestor in reversed(ancestors):
        for attribute in ancestor.attributes.values():
            if attribute.namespaceURI == "http://www.w3.org/2000/xmlns/":
                namespaces[attribute.name] = attribute.value
    clone = element.cloneNode(True)
    for name, value in namespaces.items():
        clone.setAttributeNS("http://www.w3.org/2000/xmlns/", name, value)
    return clone.toxml(encoding="utf-8")


def prepare(catalogue: common.Catalogue, working: Path) -> tuple[int, int]:
    roots, dependencies = common.resources(catalogue)
    independent_urls = {resource.url for resource in roots if resource.url}
    locations, inputs = [], []
    for url, _, path in catalogue.entries:
        locations.append(row(url, str(path)))
        data = path.read_bytes()
        common.check_xml_bytes(data)
        document = minidom.parseString(data)
        element = document.documentElement
        if element.namespaceURI == "http://schemas.xmlsoap.org/wsdl/":
            types = [child for child in element.childNodes if child.nodeType == child.ELEMENT_NODE and child.namespaceURI == element.namespaceURI and child.localName == "types"]
            for group in types:
                for schema in group.childNodes:
                    if schema.nodeType != schema.ELEMENT_NODE or schema.namespaceURI != "http://www.w3.org/2001/XMLSchema" or schema.localName != "schema":
                        continue
                    extracted = working / f"schema-{len(inputs)}.xml"
                    extracted.write_bytes(inline_schema(schema))
                    # Original owner directory remains the resolution base. The
                    # synthetic system ID is never fetched as an import.
                    inputs.append(row(path.as_uri() + f".inline-{len(inputs)}.xsd", str(extracted)))
        elif path.as_uri() in independent_urls:
            inputs.append(row(path.as_uri(), str(path)))
    (working / "catalogue.tsv").write_text("\n".join(locations), encoding="utf-8")
    (working / "roots.tsv").write_text("\n".join(inputs), encoding="utf-8")
    return len(inputs), dependencies


def run(catalogue: common.Catalogue, tool_root: Path, java: str, corpus: Path | None = None) -> str:
    jars = verify_tool(tool_root)
    with tempfile.TemporaryDirectory(prefix="xerces-run-", dir=catalogue.root) as temporary:
        working = Path(temporary)
        prepare(catalogue, working)
        if corpus is not None:
            cases = []
            for path, expected in common.corpus_cases(corpus):
                common.check_xml_bytes(path.read_bytes())
                cases.append(row(expected, str(path)))
            (working / "cases.tsv").write_text("\n".join(cases), encoding="utf-8")
        result = subprocess.run(
            [java, "--class-path", os.pathsep.join(str(path) for path in jars), str(JAVA_SOURCE),
             str(working), "compile" if corpus is None else "validate"],
            capture_output=True, text=True, timeout=120, check=False,
        )
        if result.returncode:
            diagnostic = re.fullmatch(r"FAIL: Xerces ([A-Za-z0-9_.:-]{1,120})\s*", result.stderr)
            category = diagnostic[1] if diagnostic else "backend execution failure"
            raise common.VerificationError(f"Xerces compilation/validation failed: {category}")
        if not result.stdout.startswith("PASS: Xerces ") or result.stderr:
            raise common.VerificationError("unexpected Xerces verifier output")
        return result.stdout.strip()


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["fetch-tool", "compile", "validate"])
    parser.add_argument("--tool-root", type=Path, required=True)
    parser.add_argument("--root", type=Path)
    parser.add_argument("--manifest", type=Path, default=common.MANIFEST)
    parser.add_argument("--java", default="java", help="JDK 17+ java executable, no system installation required")
    parser.add_argument("--corpus", type=Path)
    args = parser.parse_args(argv)
    if args.command != "fetch-tool" and args.root is None:
        parser.error("--root is required for compilation/validation")
    if (args.command == "validate") != (args.corpus is not None):
        parser.error("--corpus must be supplied only with validate")
    try:
        if args.command == "fetch-tool":
            fetch_tool(args.tool_root)
            print("PASS: hash-pinned Xerces verification dependencies; no schema validation performed")
        else:
            catalogue = common.Catalogue(args.root, json.loads(args.manifest.read_text(encoding="utf-8")))
            print(run(catalogue, args.tool_root, args.java, args.corpus))
        return 0
    except Exception as error:
        message = str(error) if isinstance(error, common.VerificationError) else type(error).__name__
        print(f"FAIL: {message}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
