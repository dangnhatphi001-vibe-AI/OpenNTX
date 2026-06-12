pub fn render(version: &str, registered_app_count: usize) -> String {
    format!(
        "\
OpenNTX AppPortal V{version}
Windows apps, Linux soul.

Registered apps: {registered_app_count}
Runtime: not implemented yet

[1] Library
[2] Analyze EXE
[3] Write Install Plan
[4] Desktop Launcher
[5] Settings
[Q] Quit

Security: unknown EXE files should stay sandboxed and must not run automatically."
    )
}
