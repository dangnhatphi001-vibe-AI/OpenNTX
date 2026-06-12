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
openntx run <app-id>
```

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
- App menu visibility controls.
- Uninstall metadata.
- Taskbar/window identity.
- Icon extraction and conversion.
