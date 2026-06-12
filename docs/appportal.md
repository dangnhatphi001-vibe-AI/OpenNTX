# AppPortal

AppPortal is the user-friendly frontend for OpenNTX. It is not the runtime and must not contain compatibility-layer logic.

V0.4 provides a lightweight TUI/mock so the project can stay buildable without requiring GTK4 or libadwaita development packages.

## Home

- Large drop area: "Drop a Windows .exe installer here".
- "Choose EXE" action.
- Recent apps list.
- Security reminder.

## File Analysis

After selecting an EXE, AppPortal should show:

- file name
- detected type
- architecture
- generated manifest plan
- recommended action
- security warning

Recommended actions:

- Run Once
- Install App
- Package to DEB

## Install Wizard

1. Analyze.
2. Generate manifest.
3. Review sandbox permissions.
4. Create launcher plan.
5. Run installer capture in a future module.
6. Select main executable after future capture.
7. Done.

## App Library

The app library should list installed OpenNTX apps with actions:

- Run
- Settings
- Repair
- Package
- Remove

V0.4 AppPortal mock reads registered apps from:

```text
~/.local/share/openntx/apps/
```

## Settings

- default sandbox profile
- runtime backend
- compatibility database setting
- diagnostics and logs

## Background Services

V0.4 does not require a heavy background daemon. Future background services must have a documented reason, narrow permissions, and clear diagnostics.
