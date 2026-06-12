pub fn render() -> String {
    [
        "Settings",
        "- Default sandbox profile: standard",
        "- Runtime backend: not-implemented",
        "- Manifest generation: enabled",
        "- App registry: ~/.local/share/openntx/apps/",
        "- Compatibility database: local profiles planned",
        "- Diagnostics/logs: ~/.local/state/openntx/logs/",
    ]
    .join("\n")
}
