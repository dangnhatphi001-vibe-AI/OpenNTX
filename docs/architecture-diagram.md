# Architecture Diagram

OpenNTX is core-first. The GUI is a frontend. Runtime execution is not implemented in V0.5.

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
  +-- drag/drop EXE --> AppPortal TUI/mock ----------------+
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
            V0.5 only                 future placeholder                  future PE/NT/Win32 research
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

## V0.5 Boundary

V0.5 validates inputs, parses PE metadata, generates manifests, writes app registry plans, writes desktop launchers, and tests the project foundation. It does not execute Windows binaries or installers.
