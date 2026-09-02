# Security policy

## Supported versions

| Version | Security fixes |
| --- | --- |
| 0.16.x | Supported |
| oxvif-cli 0.1.x | Supported |
| 0.15.x and older | Not supported |
| Development branches / untagged builds | Preview only; not a supported release |

This table is updated at each public release. Commercial support, if offered,
is governed by its separate written support policy and does not expand the
open-source support window implicitly.

## Reporting a vulnerability

Please do not open a public issue for a suspected vulnerability. Use GitHub's
[private vulnerability report](https://github.com/smiti1642/oxvif/security/advisories/new)
and include the affected version, platform, impact, minimal reproduction, and
whether credentials or device mutation are involved. Do not send live camera
credentials, private keys, or unsanitized captures.

We aim to acknowledge a complete report within five business days, establish
severity and next steps within ten business days, and coordinate disclosure
after a fix or mitigation is available. These are response targets, not a
guaranteed service-level agreement.

## Security boundaries

- The CLI never accepts passwords in URL userinfo and never stores secrets in
  `devices.toml`.
- `oxvif-cli 0.1` uses Windows Credential Manager, macOS Keychain, or Linux
  Secret Service according to the host platform. No platform falls back to
  plaintext storage.
- Diagnostic output and verbose stderr must not contain passwords,
  authorization headers, WS-Security material, or URI userinfo.
- Read-only diagnostics are not a guarantee that a device or network is safe.
