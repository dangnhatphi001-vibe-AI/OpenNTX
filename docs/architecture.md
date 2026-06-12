# Architecture

OpenNTX uses a core-first architecture. The CLI and AppPortal are frontends. OpenNTX Core owns data models, validation, path layout, runtime planning, and desktop integration.

```text
app.exe
  |
  v
PE detection
  |
  v
OpenNTX Core
  |
  +--> manifest resolver
  +--> compatibility profile lookup
  +--> sandbox policy resolver
  +--> registry overlay model
  +--> filesystem mapping model
  +--> desktop integration
  |
  v
runtime backend
  |
  +--> NotImplementedBackend        (V0.1)
  +--> ExternalCompatibilityBackend (future placeholder)
  +--> FutureNativeBackend          (future PE/NT/Win32 research)
  |
  v
Linux backend services
  |
  v
app process
```

## Modules

- `openntx-core`: reusable engine and models.
- `openntx-cli`: command-line interface for automation and diagnostics.
- `openntx-appportal`: lightweight user-facing frontend.
- `schemas`: JSON schemas for portable data formats.
- `examples`: reference manifests, capture reports, and desktop entries.
- `docs`: design and policy documentation.

## Boundaries

The GUI must not contain runtime logic. AppPortal calls core APIs or CLI-compatible flows. Runtime backends must expose explicit plans and errors instead of silently falling back to unsupported behavior.

## V0.1 Runtime State

Runtime execution is intentionally not implemented. The only runtime backend available in V0.1 returns a not-implemented plan.
