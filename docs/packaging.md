# Packaging

OpenNTX can package captured Windows applications into Linux-native-feeling `.deb` packages.

## V0.9 Implementation

V0.9 adds a .deb package builder prototype for registered OpenNTX apps.

### CLI Commands

```bash
openntx package build <app-id>                    # dry-run (default)
openntx package build <app-id> --yes              # actually build
openntx package build <app-id> --output dist      # custom output dir
openntx package build <app-id> --version 1.0.0    # custom version
openntx package build <app-id> --dry-run           # explicit dry-run
```

### Package Layout

```text
<staging>/
  DEBIAN/control
  opt/openntx/apps/<app-id>/
    manifest.json
    install-plan.json
    metadata.json
    drive_c/
    registry/
    capture/
  usr/share/applications/openntx-<app-id>.desktop
```

### Package Naming

```text
openntx-<app-id>_<version>_all.deb
```

### DEBIAN/control

Generated control file includes:
- Package: openntx-<app-id>
- Version: <version>
- Architecture: all
- Depends: openntx-cli
- Description: OpenNTX managed Windows application

### Security

- Does not execute EXE files or installers.
- Rejects symlinks pointing outside the app directory.
- Validates all file paths with `symlink_metadata()` before copying.
- Does not follow symlinks during directory scanning.
- Does not modify system package database.
- Does not install the .deb automatically.
- Builds with `dpkg-deb --root-owner-group` so package contents are owned by root:root.
- Sets 0644 permissions on regular data files (.json, .txt, .desktop).
- Sets 0755 permissions on directories.

### Dependency Model

Generated app packages depend on:

```text
openntx-cli
```

This is a future packaging metadata dependency. The `openntx-cli` package may not exist as a real system package yet.

## Uninstall Behavior

Package removal should remove packaged files and desktop entries. User state should follow a documented policy and should not be deleted silently without user consent.

## Limitations

- V0.9 builds .deb packages using `dpkg-deb` if available on the system.
- If `dpkg-deb` is missing, the build fails with a clear error message.
- Icons are not yet embedded in packages.
- Runtime dependency `openntx-cli` may not exist as a real system package.
