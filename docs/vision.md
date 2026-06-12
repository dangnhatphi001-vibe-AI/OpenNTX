# Vision

OpenNTX exists to make Windows desktop applications feel like first-class Linux desktop citizens.

The long-term target is simple:

```text
Drop EXE. Run Native.
```

The user should not need to understand prefixes, wrapper scripts, virtual machines, compatibility launchers, or manual command lines. A Windows installer should become a managed Linux desktop application with a launcher, icon, manifest, isolated state, logs, uninstall metadata, and a clear permission model.

OpenNTX will only earn that experience by building core systems first:

- PE/EXE analysis.
- Manifest-driven app identity.
- Per-app filesystem and registry overlays.
- Installer capture.
- Desktop integration.
- Runtime backend abstraction.
- Security and sandboxing from the start.

V0.2 is still a foundation. It analyzes PE metadata but does not run arbitrary Windows applications.
