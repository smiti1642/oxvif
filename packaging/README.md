# oxvif distribution staging

This directory contains reviewable inputs for the first three-platform CLI
release. It is not evidence that a public package channel exists.

- `oxvif.1` is the packaged manual page.
- `debian/control.in` is rendered with the CLI version and Debian architecture
  by the release workflow. The project package has no maintainer scripts.
- `homebrew/oxvif.rb.in` is rendered only after the matching macOS archives and
  SHA-256 hashes exist. It targets the project tap; an eventual Homebrew Core
  submission will build from source and follow the policy current at submission
  time.
- `create_archive.py` creates sorted, owner-normalized `.tar.gz` and `.zip`
  archives using the tag commit timestamp so repeated builds in the same pinned
  environment produce the same bytes.

Publishing an APT repository, signing key, tap, formula, bottle, tag, crate, or
GitHub Release requires the release approval defined in the active
three-platform distribution plan.

The release workflow may be dispatched against a branch or commit only with
`publish=false` to validate temporary Actions artifacts. Publication mode
requires the exact version tag and rejects release notes still marked
unreleased. A `develop` push that changes `packaging/` also runs the same
non-publishing staging workflow against the exact pushed commit.


`check_keyring_migration.py --run-native` is an opt-in native credential probe.
It writes only one fresh synthetic account using exact keyring 3.6.3/4.2.0 in a
temporary consumer project, checks cross-version reads/updates/deletes, and cleans
up. Production dependencies are unchanged. Run each supported OS in a disposable
native store session; a Windows pass does not establish other platform behavior.
See the [acceptance register](../docs/active/remaining-plan-acceptance.md).

The offline `snapshot_probe` example reads a captured body and reports bounded
signature evidence without network access, body output, decoding or source edits:
`cargo run --example snapshot_probe --features health -- /path/to/captured-body`.
It retains the production signature policy; diagnostic CR/LF trimming is reported
separately and does not authorize accepting a camera response.
