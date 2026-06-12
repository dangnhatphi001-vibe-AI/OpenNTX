# Installer Capture

Installer Capture Mode is the planned process for turning a Windows installer into an OpenNTX-managed application.

## Planned Flow

1. Create a temporary install environment.
2. Snapshot filesystem state.
3. Snapshot registry overlay state.
4. Run installer inside a controlled runtime backend.
5. Snapshot filesystem state again.
6. Snapshot registry overlay state again.
7. Diff filesystem changes.
8. Diff registry changes.
9. Detect shortcuts.
10. Identify candidate main executables.
11. Generate a capture report.
12. Generate or refine an app manifest.
13. Generate a Linux launcher.
14. Optionally package the captured app into a `.deb`.

## Capture Report

Capture reports are structured JSON documents containing:

- files created
- files modified
- registry keys created
- registry values changed
- shortcuts detected
- executable candidates
- installer exit code
- warnings
- errors

## V0.7 Limitation

V0.7 can generate an initial manifest from PE metadata before capture. It still does not execute installers or perform real filesystem/registry diff capture.
