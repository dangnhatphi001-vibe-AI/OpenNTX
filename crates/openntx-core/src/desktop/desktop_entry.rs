use crate::manifest::AppManifest;
use crate::{OpenNtxError, Result};

pub fn generate_desktop_entry(manifest: &AppManifest, cli_command: &str) -> Result<String> {
    if manifest.app_id.trim().is_empty() {
        return Err(OpenNtxError::InvalidInput(
            "manifest app_id is required for desktop entry generation".to_string(),
        ));
    }

    let name = desktop_value(&manifest.desktop.name);
    let icon = desktop_value(&manifest.desktop.icon);
    let categories = if manifest.desktop.categories.is_empty() {
        "Utility;".to_string()
    } else {
        format!(
            "{};",
            manifest
                .desktop
                .categories
                .iter()
                .map(|value| desktop_value(value))
                .collect::<Vec<_>>()
                .join(";")
        )
    };

    Ok(format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name={name}\n\
         Comment=Windows application managed by OpenNTX\n\
         Exec={cli_command} run {}\n\
         Icon={icon}\n\
         Categories={categories}\n\
         StartupNotify=true\n\
         NoDisplay=false\n",
        manifest.app_id
    ))
}

fn desktop_value(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            '\n' | '\r' | '\t' => ' ',
            _ => ch,
        })
        .collect::<String>()
        .trim()
        .to_string()
}
