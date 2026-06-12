pub fn analyze_flow_intro() -> String {
    [
        "OpenNTX Analyze EXE",
        "-------------------",
        "Enter a Windows PE/EXE path to inspect metadata.",
        "No Windows binary will be executed.",
    ]
    .join("\n")
}

pub fn install_plan_intro() -> String {
    [
        "OpenNTX Install Plan",
        "--------------------",
        "Enter a Windows PE/EXE path to generate and optionally write an OpenNTX app registry entry.",
        "This creates metadata only and does not run installers.",
    ]
    .join("\n")
}
