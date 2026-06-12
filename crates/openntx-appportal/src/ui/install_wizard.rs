pub fn render() -> String {
    [
        "Install Wizard",
        "1. Analyze EXE",
        "2. Generate manifest",
        "3. Review sandbox",
        "4. Create launcher plan",
        "5. Optionally write .desktop launcher",
        "6. Capture install remains a future module",
        "7. Select main executable after future capture",
        "8. Done",
        "",
        "V0.5 status: mock only. Registry plan and launcher writing are available; installer execution and capture are not implemented.",
    ]
    .join("\n")
}
