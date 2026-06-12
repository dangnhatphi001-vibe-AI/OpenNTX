use crate::packaging::layout::DebPackageLayout;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebPackagePlan {
    pub layout: DebPackageLayout,
    pub status: String,
}

impl DebPackagePlan {
    pub fn dry_run(app_id: impl Into<String>) -> Self {
        Self {
            layout: DebPackageLayout::new(app_id),
            status: "dry-run / package builder not implemented in V0.8".to_string(),
        }
    }
}
