# Architecture Diagram

OpenNTX is core-first. The GUI is a frontend. Runtime execution is not implemented in V0.8.

```text
                                  Linux Desktop
                                      |
                                      v
                         .desktop entry / app menu
                                      |
                                      v
                                  openntx run


User input
  |
  +-- terminal input --> AppPortal registry TUI -----------+
  |                                                        |
  +-- CLI command ----> openntx analyze/install/run -------+
                                                           |
                                                           v
                                                    OpenNTX Core
                                                           |
          +----------------+---------------+---------------+----------------+
          |                |               |               |                |
          v                v               v               v                v
      PE analyzer   manifest generator  app registry  desktop model   packaging plan
          |                |               |               |                |
          +----------------+---------------+---------------+----------------+
                                                           |
                                                           v
                                             runtime backend abstraction
                                                           |
                +--------------------------+----------------+--------------------------+
                |                          |                                           |
                v                          v                                           v
      NotImplementedBackend       ExternalCompatibilityBackend                FutureNativeBackend
            V0.8 only                 future placeholder                  future PE/NT/Win32 research
                |
                v
       dry-run plan / clear error
```

## Data Layout

```text
~/.local/share/openntx/
  apps/<app-id>/
    manifest.json
    drive_c/
    registry/

~/.local/state/openntx/logs/
~/.cache/openntx/
~/.local/share/applications/openntx-<app-id>.desktop
~/.local/share/icons/hicolor/
```

## V0.8 Boundary

V0.8 validates inputs, parses PE metadata, generates manifests, writes app registry plans, writes desktop launchers, implements capture snapshot/diff infrastructure, exposes a registry-backed AppPortal TUI, and tests the project foundation. It does not execute Windows binaries or installers.
