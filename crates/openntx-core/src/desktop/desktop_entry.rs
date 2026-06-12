use crate::app_id::is_valid_app_id;
use crate::manifest::AppManifest;
use crate::{OpenNtxError, Result};

pub fn generate_desktop_entry(manifest: &AppManifest, cli_command: &str) -> Result<String> {
    if !is_valid_app_id(&manifest.app_id) {
        return Err(OpenNtxError::InvalidInput(
            "valid manifest app_id is required for desktop entry generation".to_string(),
        ));
    }
    if cli_command.trim().is_empty() || cli_command.contains('\n') || cli_command.contains('\r') {
        return Err(OpenNtxError::InvalidInput(
            "desktop entry CLI command must be a single non-empty command".to_string(),
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
         Exec={} run {}\n\
         Icon={icon}\n\
         Categories={categories}\n\
         StartupNotify=true\n\
         NoDisplay=false\n",
        desktop_exec_token(cli_command),
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

fn desktop_exec_token(value: &str) -> String {
    value
        .chars()
        .filter(|ch| !matches!(ch, '\n' | '\r' | '\t'))
        .collect::<String>()
        .trim()
        .to_string()
}
