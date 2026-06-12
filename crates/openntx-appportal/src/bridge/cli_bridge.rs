#[derive(Debug, Default)]
pub struct CliBridge;

impl CliBridge {
    pub fn preview_install_commands(&self, input: &str) -> Vec<String> {
        vec![
            format!("openntx analyze {input}"),
            format!("openntx install {input}"),
            "openntx doctor <generated-app-id>".to_string(),
        ]
    }
}
