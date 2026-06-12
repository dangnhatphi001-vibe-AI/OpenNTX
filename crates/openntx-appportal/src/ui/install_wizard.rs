pub fn render() -> String {
    [
        "Install Wizard",
        "1. Analyze EXE",
        "2. Generate manifest",
        "3. Review sandbox",
        "4. Create launcher plan",
        "5. Capture install remains a future module",
        "6. Select main executable after future capture",
        "7. Done",
        "",
        "V0.4 status: mock only. Registry plan writing is available; installer execution and capture are not implemented.",
    ]
    .join("\n")
}
