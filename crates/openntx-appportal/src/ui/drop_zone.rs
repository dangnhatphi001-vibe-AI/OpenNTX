pub fn render() -> String {
    [
        "Drop Zone",
        "- Analyze selected file",
        "- Generate manifest metadata",
        "- Show file name",
        "- Show detected PE type",
        "- Show imported DLL summary when available",
        "- Recommend Run Once, Install App, or Package to DEB",
        "- Show security warning before any future execution",
    ]
    .join("\n")
}
