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
