# Roadmap

OpenNTX is ambitious but must remain honest about what is implemented.

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

- Read DOS header.
- Read PE signature.
- Read COFF header.
- Read optional header.
- Detect machine architecture.
- Detect subsystem type.
- Parse imported DLL names when simple and safe.
- Generate initial app manifest from PE metadata.

## Phase 2: AppPortal Drag-and-Drop Install Flow Mock

- Native Linux GUI or lightweight frontend.
- Drag/drop and file picker.
- Analysis result screen.
- Install wizard screens.
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

1. Implement a real PE analyzer for DOS header, PE signature, COFF header, optional header, machine architecture, subsystem type, and simple imported DLL names.
2. Implement manifest generation from PE analysis.
3. Implement desktop entry generation from manifest.
4. Connect AppPortal file picker/drop flow to the analyze command.
5. Implement dry-run installer plan.
6. Implement capture report generator mock.
7. Implement real filesystem diff capture in a temporary directory.
8. Implement packaging prototype for `.deb` layout.
9. Implement runtime backend interface with NotImplementedBackend, ExternalCompatibilityBackend, and FutureNativeBackend.
10. Start a research branch for actual PE loading and NT runtime concepts.
