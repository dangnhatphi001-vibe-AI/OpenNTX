pub fn render() -> String {
    [
        "Drop Zone",
        "- Analyze selected file",
        "- Show file name",
        "- Show detected PE type",
        "- Recommend Run Once, Install App, or Package to DEB",
        "- Show security warning before any future execution",
    ]
    .join("\n")
}
