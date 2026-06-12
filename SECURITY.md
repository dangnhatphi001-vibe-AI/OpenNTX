# Security Policy

Windows EXE files are untrusted input. OpenNTX must treat installers and applications as potentially malicious unless the user explicitly trusts them.

## V0.1 Status

OpenNTX V0.4 does not execute Windows binaries. It analyzes PE metadata, generates manifests, writes local registry plans, validates input, models app state, and documents future runtime architecture.

## Security Principles

- Do not run unknown EXE files with full home access by default.
- Do not auto-run installers without user confirmation.
- Do not silently grant network access.
- Keep per-app data isolated.
- Keep logs for debugging and incident review.
- Prefer deny-by-default or ask-by-default permissions.
- Future runtime work should integrate Linux sandboxing technologies.

## Threat Model Categories

- Malicious installers.
- Network-capable Windows applications.
- Credential theft attempts.
- Filesystem escape attempts.
- Registry persistence.
- Process injection into other app sessions.
- Abusive file associations.
- Supply-chain tampering.
- Misleading compatibility profiles.

## Future Sandbox Goals

Future versions should evaluate:

- Linux namespaces.
- seccomp filters.
- bubblewrap-like launch isolation.
- xdg-desktop-portal permission prompts.
- per-app network policy.
- per-app document/downloads mediation.
- device access mediation for camera, microphone, and removable drives.

## Vulnerability Reporting

Report vulnerabilities privately to the project owner or maintainers. Include:

- affected commit or release
- reproduction steps
- expected impact
- logs or generated manifests if relevant
- whether an EXE sample is required to reproduce

Do not publish exploit details before maintainers have had time to respond.

## Licensing Note

The source is available under a noncommercial default license. Commercial security services, commercial redistribution, production deployment, paid support, paid packaging, or integration into commercial products require written permission from Đặng Nhất Phi.
