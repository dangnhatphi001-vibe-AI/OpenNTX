#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MimeAssociationPlan {
    pub app_id: String,
    pub mime_types: Vec<String>,
    pub status: String,
}

impl MimeAssociationPlan {
    pub fn planned(app_id: impl Into<String>, mime_types: Vec<String>) -> Self {
        Self {
            app_id: app_id.into(),
            mime_types,
            status: "future MIME association module".to_string(),
        }
    }
}
