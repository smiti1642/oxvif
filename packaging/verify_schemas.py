"""External-only, pinned XSD 1.1 verification tooling (not ONVIF certification).

The repository contains generic tooling and URL/hash metadata, never schemas,
derived schema indexes, or schema-derived fixtures. Download and verification
are separate commands. Verification has no network fallback.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.metadata
import json
from pathlib import Path
import re
import sys
from urllib.parse import urljoin, urlsplit
from urllib.request import FileHandler, HTTPRedirectHandler, OpenerDirector, build_opener
import warnings
from xml.parsers import expat

REPOSITORY = Path(__file__).resolve().parent.parent
MANIFEST = Path(__file__).with_name("schema-sources.json")
MAX_BYTES = 8 * 1024 * 1024
XS = "{http://www.w3.org/2001/XMLSchema}"
WSDL = "{http://schemas.xmlsoap.org/wsdl/}"
XML_BASE = "{http://www.w3.org/XML/1998/namespace}base"


class VerificationError(Exception):
    """Sanitized, fail-closed verification failure."""


def external_root(path: Path, repository: Path = REPOSITORY) -> Path:
    root, repository = path.resolve(), repository.resolve()
    if root.is_relative_to(repository) or repository.is_relative_to(root):
        raise VerificationError("resource directory must be outside, not an ancestor of, the repository")
    return root


def source_path(root: Path, url: str) -> Path:
    parts = urlsplit(url)
    if (parts.scheme != "https" or not parts.hostname or parts.netloc != parts.hostname
            or parts.query or parts.fragment or not parts.path.startswith("/")
            or not re.fullmatch(r"[a-z0-9.-]+", parts.hostname)
            or not re.fullmatch(r"/[A-Za-z0-9_./-]+", parts.path)
            or any(x in {"", ".", ".."} for x in parts.path[1:].split("/"))):
        raise VerificationError("invalid source URL in manifest")
    candidate = root / parts.hostname / parts.path[1:]
    if not candidate.resolve().is_relative_to(root.resolve()):
        raise VerificationError("source path escapes external resource directory")
    return candidate


class Catalogue:
    def __init__(self, root: Path, manifest: dict):
        self.root = external_root(root)
        if manifest.get("format") != 1 or not isinstance(manifest.get("sources"), list) or not manifest["sources"]:
            raise VerificationError("unsupported or empty source manifest")
        self.entries: list[tuple[str, str, Path]] = []
        self.locations: dict[str, str] = {}
        self.files: set[str] = set()
        self.relative_references: set[str] = set()
        for entry in manifest["sources"]:
            url, digest = entry.get("url", ""), entry.get("sha256", "")
            if not isinstance(url, str) or not isinstance(digest, str) or not re.fullmatch(r"[0-9a-f]{64}", digest):
                raise VerificationError("invalid source digest or URL")
            path = source_path(self.root, url)
            local = path.as_uri()
            if url in self.locations or local in self.files:
                raise VerificationError("duplicate source or local path")
            self.entries.append((url, digest, path))
            # Legacy HTTP references map only to these exact pinned HTTPS sources.
            self.locations[url] = local
            self.locations["http:" + url[6:]] = local
            self.locations[local] = local
            self.files.add(local)

    def resolve(self, uri: str) -> str:
        # xmlschema calls the mapper BEFORE resolving a relative location. Keep
        # only reviewed relative references; the file-only opener checks the
        # final absolute URL against the exact catalogue, using its owner base.
        if uri in self.relative_references:
            return uri
        try:
            return self.locations[uri]
        except KeyError:
            # Do not echo arbitrary URLs (they can contain credentials).
            raise VerificationError("unlisted schema resource requested") from None

    def verify_files(self) -> None:
        for url, digest, path in self.entries:
            source_path(self.root, url)  # Recheck containment after creation.
            if not path.is_file() or path.stat().st_size > MAX_BYTES:
                raise VerificationError("missing or oversized pinned resource")
            if hashlib.sha256(path.read_bytes()).hexdigest() != digest:
                raise VerificationError("pinned resource digest mismatch")

    def fetch(self) -> None:
        opener = build_opener(HttpsRedirects())
        for url, digest, path in self.entries:
            if path.exists():
                if not path.is_file() or path.stat().st_size > MAX_BYTES or hashlib.sha256(path.read_bytes()).hexdigest() != digest:
                    raise VerificationError("existing resource differs; refusing to overwrite")
                continue
            with opener.open(url, timeout=30) as response:
                if urlsplit(response.url).scheme != "https":
                    raise VerificationError("non-HTTPS download refused")
                data = response.read(MAX_BYTES + 1)
            if len(data) > MAX_BYTES or hashlib.sha256(data).hexdigest() != digest:
                raise VerificationError("download size or digest mismatch")
            path.parent.mkdir(parents=True, exist_ok=True)
            source_path(self.root, url)
            with path.open("xb") as output:
                output.write(data)
        self.verify_files()

    def opener(self) -> OpenerDirector:
        # Do not use build_opener here: its defaults include network handlers.
        opener = OpenerDirector()
        opener.add_handler(PinnedFiles(self))
        return opener


class PinnedFiles(FileHandler):
    def __init__(self, catalogue: Catalogue):
        super().__init__()
        self.catalogue = catalogue

    def file_open(self, request):
        if request.full_url not in self.catalogue.files:
            raise VerificationError("unlisted absolute schema file requested")
        return super().file_open(request)


class HttpsRedirects(HTTPRedirectHandler):
    def redirect_request(self, request, fp, code, message, headers, newurl):
        if urlsplit(newurl).scheme != "https":
            raise VerificationError("non-HTTPS redirect refused")
        return super().redirect_request(request, fp, code, message, headers, newurl)


def runtime():
    for name, required in [("xmlschema", "4.3.2"), ("elementpath", "5.1.4")]:
        if importlib.metadata.version(name) != required:
            raise VerificationError("verification dependency version mismatch")
    import xmlschema
    return xmlschema


def check_xml_bytes(data: bytes) -> None:
    if len(data) > MAX_BYTES:
        raise VerificationError("oversized XML input")
    parser = expat.ParserCreate()

    def reject_doctype(*args):
        raise VerificationError("DTD is forbidden in verification inputs")

    parser.StartDoctypeDeclHandler = reject_doctype
    parser.Parse(data, True)


def resources(catalogue: Catalogue):
    """Check closure and retain per-element namespaces without editing sources."""
    xmlschema = runtime()
    catalogue.verify_files()
    result = []
    imported = set()
    dependencies = 0
    for url, _, path in catalogue.entries:
        data = path.read_bytes()
        check_xml_bytes(data)
        resource = xmlschema.XMLResource(path.as_uri(), allow="local", defuse="always", uri_mapper=catalogue.resolve, opener=catalogue.opener())
        if resource.root.tag not in {XS + "schema", WSDL + "definitions"}:
            raise VerificationError("resource is neither an XSD nor a WSDL document")
        for node in resource.root.iter():
            if XML_BASE in node.attrib:
                raise VerificationError("xml:base needs explicit resolver qualification")
            field = None
            if node.tag in {XS + n for n in ("import", "include", "redefine", "override")}:
                field = "schemaLocation"
            elif node.tag == WSDL + "import":
                field = "location"
            if field:
                location = node.get(field)
                if not location:
                    # This gate requires an explicit closed catalogue. Do not
                    # silently let a missing hint load vendor/builtin fallbacks.
                    raise VerificationError("schema dependency has no explicit location")
                resolved = catalogue.resolve(urljoin(url, location))
                if field == "schemaLocation":
                    imported.add(resolved)
                if not urlsplit(location).scheme:
                    catalogue.relative_references.add(location)
                dependencies += 1
        if resource.root.tag == XS + "schema":
            result.append(resource)
        else:
            inline = resource.root.findall(WSDL + "types/" + XS + "schema")
            if not inline:
                raise VerificationError("WSDL has no inline schema to verify")
            result.extend(resource.subresource(element) for element in inline)
    # Imported XSDs are loaded by their owning schemas. Passing the same file
    # again as an explicit root would register it twice. WSDL subresources have
    # no separate document URL and retain their in-scope namespace declarations.
    roots = [resource for resource in result if resource.url not in imported]
    if not roots:
        raise VerificationError("catalogue has no independent schema roots")
    return roots, dependencies


def compile_catalogue(catalogue: Catalogue):
    xmlschema = runtime()
    with warnings.catch_warnings():
        warnings.simplefilter("error")
        inputs, dependencies = resources(catalogue)
        schema = xmlschema.XMLSchema11(
            inputs, validation="strict", allow="local", defuse="always",
            uri_mapper=catalogue.resolve, use_fallback=False, use_cache=False,
            opener=catalogue.opener(),
        )
    return schema, len(inputs), dependencies


def validate_instance(schema, data: bytes, expected_root: str, payload_path: list[str] | None = None) -> None:
    """Validate a complete explicitly anchored instance, ignoring location hints."""
    xmlschema = runtime()
    check_xml_bytes(data)
    resource = xmlschema.XMLResource(data, allow="none", defuse="always")
    if resource.root.tag != expected_root:
        raise VerificationError("instance root differs from corpus expectation")
    if expected_root not in schema.maps.elements:
        raise VerificationError("instance root has no schema declaration")
    with warnings.catch_warnings():
        warnings.simplefilter("error")
        schema.validate(resource, use_defaults=False, use_location_hints=False)
        if payload_path:
            element = resource.root
            for index, name in enumerate(payload_path):
                matches = [child for child in element if child.tag == name]
                if len(matches) != 1 or (index == len(payload_path) - 1 and len(element) != 1):
                    raise VerificationError("missing or ambiguous corpus payload path")
                element = matches[0]
            if element.tag not in schema.maps.elements:
                raise VerificationError("corpus payload has no schema declaration")
            schema.validate(resource.subresource(element), use_defaults=False, use_location_hints=False)


def corpus_cases(directory: Path) -> list[tuple[Path, str, list[str]]]:
    directory = external_root(directory)
    index = directory / "cases.json"
    if not index.is_file() or index.stat().st_size > MAX_BYTES:
        raise VerificationError("missing or oversized corpus index")
    manifest = json.loads(index.read_text(encoding="utf-8"))
    cases = manifest.get("cases")
    if manifest.get("format") != 1 or not isinstance(cases, list) or not 0 < len(cases) <= 10000:
        raise VerificationError("unsupported or empty corpus")
    seen = set()
    result = []
    for case in cases:
        filename, expected = case.get("file", ""), case.get("root", "")
        payload_path = case.get("payload_path", [])
        if (not isinstance(filename, str) or not re.fullmatch(r"[A-Za-z0-9_-]+\.xml", filename)
                or filename in seen or not isinstance(expected, str) or not expected):
            raise VerificationError("invalid or duplicate corpus case")
        if (not isinstance(payload_path, list) or len(payload_path) > 8
                or any(not isinstance(name, str) or not name or len(name) > 1024 for name in payload_path)):
            raise VerificationError("invalid corpus payload path")
        seen.add(filename)
        path = directory / filename
        if not path.resolve().is_relative_to(directory) or not path.is_file() or path.stat().st_size > MAX_BYTES:
            raise VerificationError("missing, escaping or oversized corpus file")
        result.append((path, expected, payload_path))
    return result


def validate_corpus(schema, directory: Path) -> int:
    cases = corpus_cases(directory)
    for path, expected, payload_path in cases:
        validate_instance(schema, path.read_bytes(), expected, payload_path)
    return len(cases)


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["fetch", "check", "compile", "validate"])
    parser.add_argument("--root", required=True, type=Path, help="dedicated directory outside the checkout")
    parser.add_argument("--manifest", type=Path, default=MANIFEST)
    parser.add_argument("--corpus", type=Path, help="external cases.json plus XML files; required by validate")
    args = parser.parse_args(argv)
    if (args.command == "validate") != (args.corpus is not None):
        parser.error("--corpus must be supplied only with validate")
    try:
        catalogue = Catalogue(args.root, json.loads(args.manifest.read_text(encoding="utf-8")))
        if args.command == "fetch":
            catalogue.fetch()
            print(f"PASS: {len(catalogue.entries)} pinned resources downloaded/verified; no instance validation performed")
        elif args.command == "check":
            roots, dependencies = resources(catalogue)
            print(f"PASS: closed catalogue; {len(catalogue.entries)} files, {len(roots)} independent schema roots, {dependencies} dependency edges; no schema compilation or instance validation performed")
        else:
            schema, count, dependencies = compile_catalogue(catalogue)
            if args.command == "validate":
                cases = validate_corpus(schema, args.corpus)
                print(f"PASS: {cases} explicitly anchored instances validated using strict XSD 1.1; not semantic conformance")
            else:
                print(f"PASS: strict XSD 1.1 compilation; {len(catalogue.entries)} files, {count} schema roots, {dependencies} dependency edges; no instance validation performed")
        return 0
    except Exception as error:
        # XML validation exceptions contain schema excerpts. Never emit those in
        # CI logs/artifacts. Generic categories preserve the redaction boundary.
        message = str(error) if isinstance(error, VerificationError) else type(error).__name__
        print(f"FAIL: {message}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
