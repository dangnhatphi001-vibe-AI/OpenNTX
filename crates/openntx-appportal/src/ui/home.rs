pub fn render() -> String {
    [
        "OpenNTX AppPortal",
        "Drop EXE. Run Native.",
        "",
        "[ Drop a Windows .exe installer here ]",
        "",
        "Actions:",
        "- Choose EXE",
        "- Generate manifest",
        "- View recent apps",
        "- Open settings",
        "",
        "Security: unknown EXE files should stay sandboxed and must not run automatically.",
    ]
    .join("\n")
}
