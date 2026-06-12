# Limitations

OpenNTX V0.8 does not run arbitrary Windows software.

Important limitations:

- Full Win32 compatibility is extremely complex.
- DirectX support is future research.
- Kernel drivers are out of scope.
- Anti-cheat and protected software are out of scope.
- Malware risk exists.
- Real sandboxing is mandatory for production use.
- Installer capture snapshot/diff infrastructure is implemented in V0.8 (analysis-only, no installer execution).
- Installer execution is not implemented in V0.8.
- Registry overlay execution is not implemented in V0.8.
- Desktop launchers are modeled and generated, but they do not make Windows execution work.
- Runtime backends are placeholders.

OpenNTX must remain honest. It is a foundation for future compatibility research, not a complete subsystem today.
