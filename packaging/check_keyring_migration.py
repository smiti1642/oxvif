"""Opt-in native keyring migration probe, isolated from the workspace dependency graph.

Writes and removes one unique synthetic account under the production service name.
Requires a disposable/unlocked native credential session. Does not inspect user entries.
Passing one platform does not validate locked/denied/reconnection behavior or another OS.
"""
import argparse
from pathlib import Path
import subprocess
import sys
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run-native", action="store_true", required=True,
                        help="run against this host's native credential backend")
    parser.parse_args()
    features = {"win32": ['windows-native'], "darwin": ['apple-native'],
                "linux": ['sync-secret-service', 'crypto-rust']}
    if sys.platform not in features:
        parser.error("unsupported native platform")
    # No candidate dependency or lockfile enters the production workspace.
    with tempfile.TemporaryDirectory(prefix="oxvif-keyring-migration-") as temporary:
        root = Path(temporary)
        (root / 'src').mkdir()
        (root / 'src/main.rs').write_bytes(Path(__file__).with_name('keyring_migration_probe.rs').read_bytes())
        import json
        manifest = '\n'.join([
            '[package]', 'name = "oxvif-keyring-migration-probe"', 'version = "0.0.0"',
            'edition = "2024"', 'rust-version = "1.88"', '[workspace]', '[dependencies]',
            'keyring3 = { package = "keyring", version = "=3.6.3", default-features = false, features = '
            + json.dumps(features[sys.platform]) + ' }',
            'keyring4 = { package = "keyring", version = "=4.2.0", default-features = false, features = ["v1"] }', '',
        ])
        (root / 'Cargo.toml').write_bytes(manifest.encode())
        return subprocess.call(['rtk', 'proxy', 'cargo', 'run', '--manifest-path', str(root / 'Cargo.toml')])


if __name__ == '__main__':
    raise SystemExit(main())
