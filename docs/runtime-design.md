# Runtime Design

OpenNTX V0.8 does not implement a Windows runtime. This document defines future research modules.

## Future Layers

- PE loader.
- DLL loader.
- NT object model.
- Handle table.
- Registry service.
- Win32 API layer.
- Graphics translation.
- Audio bridge.
- Input bridge.
- Service broker.

## Runtime Backend Abstraction

The core exposes runtime backends through explicit planning and execution APIs.

Initial backend placeholders:

- `NotImplementedBackend`: V0.8 default.
- `ExternalCompatibilityBackend`: future bridge for external compatibility engines.
- `FutureNativeBackend`: future OpenNTX PE/NT/Win32 research backend.

## Constraints

- No fake native claims.
- No silent execution fallback.
- No kernel driver support.
- No anti-cheat bypass.
- No malware execution helpers.
- No credential dumping, persistence, stealth, or evasion logic.
