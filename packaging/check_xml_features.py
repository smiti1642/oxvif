"""Run the XML fixtures as a downstream consumer, with encoding off and on.

Run after `cargo fetch --locked`. Root dev-dependencies deliberately enable
quick-xml/encoding, so normal integration tests cannot exercise the off case.
The temporary consumer reuses the repository lockfile, never updates the index,
and checks that every resolved registry version came from that lockfile.
"""

import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import tomllib


def main():
    root = Path(__file__).resolve().parents[1]
    manifest = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
    locked = tomllib.loads((root / "Cargo.lock").read_text(encoding="utf-8"))
    allowed = {(p["name"], p["version"], p.get("source")) for p in locked["package"]}
    env = dict(os.environ, CARGO_TARGET_DIR=str(root / "target" / "xml-feature-probe"))
    with tempfile.TemporaryDirectory(prefix="oxvif-xml-compat-") as directory:
        probe = Path(directory)
        (probe / "Cargo.toml").write_text(
            '[package]\nname = "oxvif-xml-compat"\nversion = "0.0.0"\nedition = "2024"\n'
            '[workspace]\n[features]\nencoding = ["quick-xml/encoding"]\n'
            '[dependencies]\n'
            f'oxvif = {{ path = {json.dumps(root.as_posix())} }}\n'
            f'quick-xml = {json.dumps(manifest["dependencies"]["quick-xml"])}\n'
            '[[test]]\nname = "xml_compat"\n'
            f'path = {json.dumps((root / "tests/xml_compat.rs").as_posix())}\n',
            encoding="utf-8",
        )
        shutil.copyfile(root / "Cargo.lock", probe / "Cargo.lock")
        args = ["--manifest-path", str(probe / "Cargo.toml"), "--offline"]
        for features in [[], ["--features", "encoding"]]:
            metadata = json.loads(subprocess.check_output(
                ["cargo", "metadata", "--format-version", "1", *args, *features], env=env,
            ))
            for package in metadata["packages"]:
                if package["source"] is not None:
                    key = (package["name"], package["version"], package["source"])
                    if key not in allowed:
                        raise RuntimeError(f"consumer resolved an unlocked package: {key}")
            xml_ids = {p["id"] for p in metadata["packages"] if p["name"] == "quick-xml"}
            nodes = [n for n in metadata["resolve"]["nodes"] if n["id"] in xml_ids]
            if len(nodes) != 1 or ("encoding" in nodes[0]["features"]) != bool(features):
                raise RuntimeError("quick-xml feature graph differs from requested coverage")
            print(f"Testing quick-xml encoding={'on' if features else 'off'}", flush=True)
            subprocess.run(["cargo", "test", *args, "--locked", *features], env=env, check=True)


if __name__ == "__main__":
    main()
