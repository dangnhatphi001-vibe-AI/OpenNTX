pub fn render(version: &str, registered_app_count: usize) -> String {
    format!(
        "\
OpenNTX AppPortal V{version}
Windows apps, Linux soul.

Registered apps: {registered_app_count}
Runtime: not implemented (analysis-only)
CI/status: V1.0-alpha feature pack

--------------------------------------------
  [1] Library
  [2] Analyze EXE
  [3] Write Install Plan
  [4] Capture Tools
  [5] Package Builder
  [6] Logs
  [7] Doctor
  [8] Settings
  [Q] Quit
--------------------------------------------

Security: unknown EXE files should stay sandboxed and must not run automatically.
Capture: snapshot/diff infrastructure only. No Windows installers are executed.
Runtime: V1.0-alpha does not execute Windows applications."
    )
}
