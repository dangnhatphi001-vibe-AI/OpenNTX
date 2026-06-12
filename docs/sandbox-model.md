# Sandbox Model

OpenNTX treats Windows executables as untrusted by default.

## Principles

- Deny by default where realistic.
- Ask before granting sensitive access.
- Keep per-app state isolated.
- Do not silently grant network access.
- Do not expose the full home directory by default.
- Record diagnostics for launch and permission decisions.

## Per-App Policy

Each app manifest may define:

- network permission
- home permission
- documents permission
- downloads permission
- removable drive permission
- future camera and microphone permission
- future process isolation policy

## Filesystem Mapping

Planned mappings:

- `drive_c`: per-app virtual drive.
- `home_mapping`: denied, limited, ask, or mapped.
- `documents_access`: deny, ask, or allow.
- `downloads_access`: deny, ask, or allow.

## Future Linux Isolation

Future versions should evaluate:

- Linux namespaces.
- seccomp.
- bubblewrap-like launch wrappers.
- xdg-desktop-portal permission prompts.
- Wayland-first desktop integration.
- per-app network mediation.
