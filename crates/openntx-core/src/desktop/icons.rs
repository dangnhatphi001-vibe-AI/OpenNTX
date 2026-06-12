#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IconPlan {
    pub app_id: String,
    pub icon_name: String,
    pub status: String,
}

impl IconPlan {
    pub fn future_extraction(app_id: impl Into<String>, icon_name: impl Into<String>) -> Self {
        Self {
            app_id: app_id.into(),
            icon_name: icon_name.into(),
            status: "future PE resource extraction module".to_string(),
        }
    }
}
