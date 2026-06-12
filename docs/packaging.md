# Packaging

OpenNTX may eventually package captured Windows applications into Linux-native-feeling `.deb` packages.

## Package Layout

```text
/opt/openntx/apps/<app-id>/
  manifest.json
  drive_c/
  registry/
  metadata/

/usr/share/applications/openntx-<app-id>.desktop
/usr/share/icons/hicolor/.../<app-id>.png
```

## Dependency Model

Generated app packages should depend on:

```text
openntx-runtime >= same major version
```

Apps should share the runtime. They should not bundle the entire OpenNTX runtime unless explicitly requested for a special distribution.

## Uninstall Behavior

Package removal should remove packaged files and desktop entries. User state should follow a documented policy and should not be deleted silently without user consent.

## V0.5 Limitation

V0.5 includes layout models only. It does not build `.deb` packages.
