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

Publishing an APT repository, signing key, tap, formula, bottle, tag, crate, or
GitHub Release requires the release approval defined in the active
three-platform distribution plan.
