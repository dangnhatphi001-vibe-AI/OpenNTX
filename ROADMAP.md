# Roadmap

OpenNTX is ambitious but must remain honest about what is implemented.

## Current V0.4 Status

- Real PE analyzer implemented.
- Manifest generation from PE metadata implemented.
- Local app registry and install plan writer implemented.
- Windows runtime execution is not implemented.
- Installer execution and capture remain future modules.

## Phase 0: Concept and Repository Foundation

- Source-available project structure.
- Architecture documentation.
- Manifest schema.
- Capture report schema.
- Compatibility profile schema.
- CLI skeleton.
- AppPortal mock.
- Core models.
- Basic tests.

## Phase 1: PE Analyzer and Manifest Generator

- Implemented in V0.2/V0.3 for analysis-only metadata flows.
- Reads DOS header.
- Reads PE signature.
- Reads COFF header.
- Reads optional header.
- Detects machine architecture.
- Detects subsystem type.
- Parses imported DLL names when simple and safe.
- Generates initial app manifest from PE metadata.

## Phase 2: AppPortal Drag-and-Drop Install Flow Mock

- Native Linux GUI or lightweight frontend.
- Drag/drop and file picker.
- Analysis result screen.
- Install wizard screens.
- Registered app library backed by local app registry.
- Security prompts.

## Phase 3: Desktop Integration and Launcher Generation

- Generate `.desktop` files.
- Install launcher into user app menu.
- Extract or assign icons.
- Track uninstall metadata.
- Prepare file association model.

## Phase 4: Installer Capture Prototype

- Temporary install environment.
- Filesystem snapshot and diff.
- Registry overlay snapshot and diff.
- Shortcut detection.
- Main executable candidate scoring.
- Capture report generation.

## Phase 5: Runtime Backend Abstraction

- Backend trait and execution plan.
- NotImplementedBackend.
- ExternalCompatibilityBackend placeholder.
- FutureNativeBackend placeholder.
- Logging and diagnostics model.

## Phase 6: Experimental Win32/NT Compatibility Research

- PE loader research.
- DLL loading strategy.
- NT object model.
- Handle table.
- Registry service.
- Win32 API layer.
- Linux graphics/audio/input bridges.

## Phase 7: Compatibility Database and Profiles

- App hash recognition.
- Known fixes.
- Required runtime modules.
- Status levels.
- Versioned compatibility profiles.

## Phase 8: Sandboxed App-Store-Style UX

- Permission UI.
- Repair flows.
- Package publishing flow.
- App library metadata.
- Sandboxed update and uninstall workflows.

## Next Work Plan

1. Implement desktop entry writing from registered manifests.
2. Connect AppPortal file picker/drop flow to analyze, manifest generation, and registry plan writing commands.
3. Implement dry-run installer capture report generation.
4. Implement real filesystem diff capture in a temporary directory.
5. Implement packaging prototype for `.deb` layout.
6. Expand compatibility profile matching from generated manifest metadata.
7. Start a research branch for actual PE loading and NT runtime concepts.
