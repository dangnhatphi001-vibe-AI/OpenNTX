# Desktop Integration

OpenNTX applications should behave like Linux desktop applications where possible.

## Launcher Generation

User desktop entries:

```text
~/.local/share/applications/openntx-<app-id>.desktop
```

System package desktop entries:

```text
/usr/share/applications/openntx-<app-id>.desktop
```

The launcher should execute:

```text
openntx run <app-id> --notify
```

V0.7 writes user launchers for registered apps through the CLI and AppPortal:

```bash
openntx desktop create <app-id> --dry-run
openntx desktop create <app-id> --yes
openntx desktop remove <app-id> --dry-run
openntx desktop remove <app-id> --yes
openntx install app.exe --write-plan --desktop
```

`openntx desktop create` is dry-run by default. `--yes` is required to write the file. The generated `Exec` line intentionally calls `openntx run <app-id> --notify` so desktop launches produce clear feedback and diagnostics logs even though runtime execution is still not implemented.

AppPortal uses the same core desktop writer:

1. Select a registered app from Library or Desktop Launcher.
2. Choose Create desktop launcher.
3. Review the dry-run path.
4. Confirm writing.
5. Remove the launcher only after confirmation.

## Icons

Icons should be extracted from PE resources in a future module or assigned from package metadata.

User icon root:

```text
~/.local/share/icons/hicolor/
```

System package icon root:

```text
/usr/share/icons/hicolor/
```

## Future Work

- MIME/file associations.
- Open-with integration.
- App menu visibility controls beyond basic `.desktop` generation.
- Rich uninstall metadata.
- Taskbar/window identity.
- Icon extraction and conversion.
