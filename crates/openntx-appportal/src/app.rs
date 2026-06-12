use crate::ui::{app_library, home, install_wizard, settings};
use openntx_core::app_id::generate_app_id;
use openntx_core::manifest::{
    generate_manifest_from_pe, AppManifest, GeneratedManifest, ManifestGenerationInput,
};
use openntx_core::pe::{analyze_pe, PeAnalysis};
use openntx_core::registry::{
    AppRegistry, DesktopMode, InstallPlan, RegisteredApp, RegistrationResult, RemoveMode,
};
use openntx_core::runtime::{NotImplementedBackend, RuntimeBackend};
use openntx_core::{OpenNtxError, Result};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug)]
pub struct AppPortalApp {
    registry: AppRegistry,
}

#[derive(Debug)]
struct InstallPreview {
    analysis: PeAnalysis,
    generated: GeneratedManifest,
    install_plan: InstallPlan,
}

impl AppPortalApp {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            registry: AppRegistry::from_env()?,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        loop {
            let apps = self.registry.list_apps()?;
            clear_screen();
            println!("{}", home::render(VERSION, apps.len()));
            match prompt("Select action")?.to_ascii_lowercase().as_str() {
                "l" | "1" => self.library_screen()?,
                "a" | "2" => self.analyze_exe_screen()?,
                "i" | "3" => self.install_plan_screen()?,
                "d" | "4" => self.desktop_launcher_screen()?,
                "s" | "5" => self.settings_screen()?,
                "q" => break,
                "" => {}
                _ => pause("Unknown action.")?,
            }
        }
        Ok(())
    }

    fn library_screen(&mut self) -> Result<()> {
        loop {
            let apps = self.registry.list_apps()?;
            clear_screen();
            println!("{}", app_library::render(&apps));
            if apps.is_empty() {
                pause("No registered apps. Press Enter to return.")?;
                return Ok(());
            }

            let input = prompt("Select app number or [B] Back")?;
            if input.eq_ignore_ascii_case("b") {
                return Ok(());
            }
            let Some(index) = parse_menu_index(&input, apps.len()) else {
                pause("Invalid app selection.")?;
                continue;
            };
            let app_id = apps[index].app_id.clone();
            if self.app_details_screen(&app_id)? {
                return Ok(());
            }
        }
    }

    fn app_details_screen(&mut self, app_id: &str) -> Result<bool> {
        loop {
            let manifest = self.registry.load_manifest(app_id)?;
            let desktop_entry_path = self.registry.paths().desktop_entry_path(app_id);
            let desktop_exists = desktop_entry_path.exists();

            clear_screen();
            print_app_details(
                &manifest,
                &self.registry.paths().manifest_path(app_id),
                &desktop_entry_path,
                desktop_exists,
            );

            match prompt("Select action")?.as_str() {
                value if value.eq_ignore_ascii_case("r") => self.run_plan_screen(&manifest)?,
                value if value.eq_ignore_ascii_case("c") => self.create_desktop_screen(app_id)?,
                value if value.eq_ignore_ascii_case("x") => self.remove_desktop_screen(app_id)?,
                value if value.eq_ignore_ascii_case("d") => {
                    self.remove_app_dry_run_screen(app_id)?
                }
                value if value.eq_ignore_ascii_case("delete") => {
                    self.remove_app_confirmed_screen(app_id)?;
                    return Ok(true);
                }
                value if value.eq_ignore_ascii_case("b") => return Ok(false),
                "" => {}
                _ => pause("Unknown app action.")?,
            }
        }
    }

    fn analyze_exe_screen(&mut self) -> Result<()> {
        clear_screen();
        println!("{}", install_wizard::analyze_flow_intro());
        let input_path = prompt_path("EXE path")?;
        if input_path.as_os_str().is_empty() {
            return Ok(());
        }

        let preview = match self.prepare_install_preview(&input_path) {
            Ok(preview) => preview,
            Err(error) => {
                pause(&format!("Analysis failed: {error}"))?;
                return Ok(());
            }
        };

        loop {
            clear_screen();
            print_analysis_summary(&preview.analysis, &preview.generated);
            println!();
            println!("[M] Generate manifest preview");
            println!("[W] Write install plan");
            println!("[B] Back");

            match prompt("Select action")?.as_str() {
                value if value.eq_ignore_ascii_case("m") => {
                    let json = serde_json::to_string_pretty(&preview.generated.manifest)?;
                    clear_screen();
                    println!("OpenNTX Manifest Preview");
                    println!("-----------------------");
                    println!("{json}");
                    pause("Preview only. No files were written.")?;
                }
                value if value.eq_ignore_ascii_case("w") => {
                    self.confirm_and_write_install_plan(&preview)?;
                    return Ok(());
                }
                value if value.eq_ignore_ascii_case("b") => return Ok(()),
                "" => {}
                _ => pause("Unknown analyze action.")?,
            }
        }
    }

    fn install_plan_screen(&mut self) -> Result<()> {
        clear_screen();
        println!("{}", install_wizard::install_plan_intro());
        let input_path = prompt_path("EXE path")?;
        if input_path.as_os_str().is_empty() {
            return Ok(());
        }

        let preview = match self.prepare_install_preview(&input_path) {
            Ok(preview) => preview,
            Err(error) => {
                pause(&format!("Install plan failed: {error}"))?;
                return Ok(());
            }
        };

        clear_screen();
        print_install_plan_summary(&preview);
        if confirm("Write this OpenNTX install plan?")? {
            self.write_install_plan(&preview)?;
        } else {
            pause("Install plan not written.")?;
        }
        Ok(())
    }

    fn desktop_launcher_screen(&mut self) -> Result<()> {
        loop {
            let apps = self.registry.list_apps()?;
            clear_screen();
            println!("OpenNTX Desktop Launcher");
            println!("------------------------");
            if apps.is_empty() {
                pause("No registered apps. Press Enter to return.")?;
                return Ok(());
            }
            print_numbered_apps(&apps);

            let input = prompt("Select app number or [B] Back")?;
            if input.eq_ignore_ascii_case("b") {
                return Ok(());
            }
            let Some(index) = parse_menu_index(&input, apps.len()) else {
                pause("Invalid app selection.")?;
                continue;
            };
            let app_id = apps[index].app_id.clone();
            clear_screen();
            println!("Desktop launcher for {app_id}");
            println!("-----------------------------");
            println!("[C] Create launcher");
            println!("[x] Remove launcher");
            println!("[B] Back");
            match prompt("Select action")?.as_str() {
                value if value.eq_ignore_ascii_case("c") => self.create_desktop_screen(&app_id)?,
                value if value.eq_ignore_ascii_case("x") => self.remove_desktop_screen(&app_id)?,
                value if value.eq_ignore_ascii_case("b") => {}
                _ => pause("Unknown desktop action.")?,
            }
        }
    }

    fn settings_screen(&self) -> Result<()> {
        clear_screen();
        println!("{}", settings::render(self.registry.paths()));
        pause("Press Enter to return.")
    }

    fn run_plan_screen(&self, manifest: &AppManifest) -> Result<()> {
        let backend = NotImplementedBackend;
        let plan = backend.plan_execution(manifest);
        clear_screen();
        println!("OpenNTX Run Plan");
        println!("----------------");
        println!("App ID: {}", plan.app_id);
        println!("Executable: {}", plan.executable);
        println!("Backend: {}", plan.backend);
        println!("Status: dry-run / not implemented");
        println!();
        println!(
            "Runtime execution is not implemented in V0.6. This action only validates app metadata and prepares a future execution plan."
        );
        pause("No Windows binary was executed.")
    }

    fn create_desktop_screen(&self, app_id: &str) -> Result<()> {
        let dry_run = self
            .registry
            .create_desktop_entry(app_id, DesktopMode::DryRun, "openntx")?;
        clear_screen();
        println!("OpenNTX Desktop Create");
        println!("----------------------");
        println!("App ID: {}", dry_run.app_id);
        println!("Desktop entry: {}", dry_run.desktop_entry_path.display());
        println!("Status: planned / not written");
        println!();
        if confirm("Write this desktop launcher?")? {
            let written =
                self.registry
                    .create_desktop_entry(app_id, DesktopMode::Write, "openntx")?;
            pause(&format!(
                "Desktop launcher written: {}",
                written.desktop_entry_path.display()
            ))?;
        } else {
            pause("Desktop launcher not written.")?;
        }
        Ok(())
    }

    fn remove_desktop_screen(&self, app_id: &str) -> Result<()> {
        let dry_run = self
            .registry
            .remove_desktop_entry(app_id, DesktopMode::DryRun)?;
        clear_screen();
        println!("OpenNTX Desktop Remove");
        println!("----------------------");
        println!("App ID: {}", dry_run.app_id);
        println!("Desktop entry: {}", dry_run.desktop_entry_path.display());
        println!("Status: planned / not removed");
        println!();
        if confirm("Remove this desktop launcher?")? {
            let removed = self
                .registry
                .remove_desktop_entry(app_id, DesktopMode::Write)?;
            let status = if removed.removed {
                "removed"
            } else {
                "launcher was already missing"
            };
            pause(&format!("Desktop launcher {status}."))?;
        } else {
            pause("Desktop launcher not removed.")?;
        }
        Ok(())
    }

    fn remove_app_dry_run_screen(&self, app_id: &str) -> Result<()> {
        let plan = self.registry.remove_app(app_id, RemoveMode::DryRun)?;
        clear_screen();
        println!("OpenNTX Remove App Dry Run");
        println!("--------------------------");
        println!("App ID: {}", plan.app_id);
        println!("Would remove app directory: {}", plan.app_dir.display());
        println!("Manifest: {}", plan.manifest_path.display());
        println!("Desktop entry: {}", plan.desktop_entry_path.display());
        println!("Status: planned / not removed");
        pause("Dry run only.")
    }

    fn remove_app_confirmed_screen(&self, app_id: &str) -> Result<()> {
        clear_screen();
        println!("OpenNTX Remove App");
        println!("------------------");
        println!("This removes only the OpenNTX registry directory for:");
        println!("{app_id}");
        println!();
        let confirmation = prompt("Type the app id exactly to confirm deletion")?;
        if confirmation != app_id {
            pause("Confirmation did not match. App was not removed.")?;
            return Ok(());
        }

        let plan = self.registry.remove_app(app_id, RemoveMode::Delete)?;
        pause(&format!("Removed: {}", plan.app_dir.display()))
    }

    fn confirm_and_write_install_plan(&self, preview: &InstallPreview) -> Result<()> {
        clear_screen();
        print_install_plan_summary(preview);
        if confirm("Write this OpenNTX install plan?")? {
            self.write_install_plan(preview)?;
        } else {
            pause("Install plan not written.")?;
        }
        Ok(())
    }

    fn write_install_plan(&self, preview: &InstallPreview) -> Result<()> {
        let result = self
            .registry
            .register_plan(&preview.generated.manifest, &preview.install_plan)?;
        clear_screen();
        print_registration_result(&result);
        pause("Registry entry created. Runtime execution is still not implemented.")
    }

    fn prepare_install_preview(&self, input_path: &Path) -> Result<InstallPreview> {
        let analysis = analyze_pe(input_path)?;
        if !analysis.is_pe {
            return Err(OpenNtxError::Unsupported(format!(
                "input must be a Windows PE/EXE file; {} is {}",
                analysis.file_name, analysis.status
            )));
        }

        let display_name = input_path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("windows-app");
        let app_id = generate_app_id(display_name, Some(&input_path.display().to_string()));
        let generated = generate_manifest_from_pe(ManifestGenerationInput {
            input_path,
            analysis: &analysis,
            app_id,
        })?;
        let install_plan = self.registry.build_install_plan(
            &generated.manifest,
            analysis.subsystem.as_ref().map(ToString::to_string),
        );

        Ok(InstallPreview {
            analysis,
            generated,
            install_plan,
        })
    }
}

fn print_analysis_summary(analysis: &PeAnalysis, generated: &GeneratedManifest) {
    println!("OpenNTX Analyze EXE");
    println!("-------------------");
    println!("Input: {}", analysis.file_name);
    println!("Format: {}", analysis.format);
    println!("Architecture: {}", analysis.architecture);
    println!(
        "Subsystem: {}",
        analysis
            .subsystem
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(|| "unknown".to_string())
    );
    println!("Imported DLL count: {}", analysis.imported_dlls.len());
    println!(
        "Suggested install mode: {}",
        generated.manifest.install_mode
    );
    println!("Reason: {}", generated.install_mode_reason);
    println!("Status: analysis-only");
    println!();
    println!("No Windows binary was executed.");
}

fn print_install_plan_summary(preview: &InstallPreview) {
    println!("OpenNTX Install Plan");
    println!("--------------------");
    println!("Input: {}", preview.analysis.file_name);
    println!("App ID: {}", preview.generated.manifest.app_id);
    println!("Name: {}", preview.generated.manifest.name);
    println!("Architecture: {}", preview.generated.manifest.architecture);
    println!(
        "Subsystem: {}",
        preview
            .analysis
            .subsystem
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(|| "unknown".to_string())
    );
    println!("Install mode: {}", preview.generated.manifest.install_mode);
    println!(
        "Imported DLL count: {}",
        preview.generated.manifest.diagnostics.imported_dlls.len()
    );
    println!(
        "Sandbox profile: {}",
        preview.generated.manifest.sandbox.profile
    );
    println!("App directory: {}", preview.install_plan.app_dir);
    println!("Manifest: {}", preview.install_plan.manifest_path);
    println!(
        "Install plan: {}/install-plan.json",
        preview.install_plan.app_dir
    );
    println!("drive_c: {}", preview.install_plan.drive_c);
    println!("registry: {}", preview.install_plan.registry);
    println!("logs: {}", preview.install_plan.logs);
    println!("Status: dry-run until confirmed");
}

fn print_registration_result(result: &RegistrationResult) {
    println!("OpenNTX Install Plan Written");
    println!("----------------------------");
    println!("App ID: {}", result.app_id);
    println!("App directory: {}", result.app_dir.display());
    println!("Manifest: {}", result.manifest_path.display());
    println!("Install plan: {}", result.install_plan_path.display());
    println!("Metadata: {}", result.metadata_path.display());
    println!("drive_c: {}", result.drive_c_path.display());
    println!("registry: {}", result.registry_path.display());
    println!("logs: {}", result.logs_path.display());
}

fn print_numbered_apps(apps: &[RegisteredApp]) {
    for (index, app) in apps.iter().enumerate() {
        println!(
            "[{}] {} | {} | {} | {} | sandbox={} | dlls={} | {} | {}",
            index + 1,
            app.name,
            app.app_id,
            app.architecture,
            app.install_mode,
            app.sandbox_profile,
            app.imported_dll_count,
            if app.desktop_launcher_exists {
                "desktop=present"
            } else {
                "desktop=missing"
            },
            app.status
        );
    }
}

fn print_app_details(
    manifest: &AppManifest,
    manifest_path: &Path,
    desktop_entry_path: &Path,
    desktop_exists: bool,
) {
    println!("OpenNTX App Details");
    println!("-------------------");
    println!("App ID: {}", manifest.app_id);
    println!("Name: {}", manifest.name);
    println!("Architecture: {}", manifest.architecture);
    println!("Install mode: {}", manifest.install_mode);
    println!("Executable path: {}", manifest.executable.path);
    println!("Manifest path: {}", manifest_path.display());
    println!("Sandbox profile: {}", manifest.sandbox.profile);
    println!(
        "Imported DLLs count: {}",
        manifest.diagnostics.imported_dlls.len()
    );
    println!("Desktop entry path: {}", desktop_entry_path.display());
    println!(
        "Desktop status: {}",
        if desktop_exists { "present" } else { "missing" }
    );
    println!("Runtime status: not implemented");
    println!();
    println!("[R] Run plan");
    println!("[C] Create desktop launcher");
    println!("[x] Remove desktop launcher");
    println!("[D] Dry-run remove app");
    println!("[Delete] Remove app with confirmation");
    println!("[B] Back");
}

fn parse_menu_index(input: &str, len: usize) -> Option<usize> {
    let value = input.trim().parse::<usize>().ok()?;
    if (1..=len).contains(&value) {
        Some(value - 1)
    } else {
        None
    }
}

pub fn confirmation_is_yes(input: &str) -> bool {
    matches!(input.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

fn confirm(message: &str) -> Result<bool> {
    let input = prompt(&format!("{message} [y/N]"))?;
    Ok(confirmation_is_yes(&input))
}

fn prompt_path(label: &str) -> Result<PathBuf> {
    Ok(PathBuf::from(prompt(label)?))
}

fn prompt(label: &str) -> Result<String> {
    print!("{label}: ");
    io::stdout()
        .flush()
        .map_err(|source| OpenNtxError::io("stdout", source))?;
    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|source| OpenNtxError::io("stdin", source))?;
    Ok(input.trim().to_string())
}

fn pause(message: &str) -> Result<()> {
    println!();
    println!("{message}");
    let _ = prompt("Press Enter")?;
    Ok(())
}

fn clear_screen() {
    print!("\x1b[2J\x1b[H");
}

#[cfg(test)]
mod tests {
    use super::confirmation_is_yes;

    #[test]
    fn confirmation_accepts_only_explicit_yes() {
        assert!(confirmation_is_yes("yes"));
        assert!(confirmation_is_yes("Y"));
        assert!(!confirmation_is_yes(""));
        assert!(!confirmation_is_yes("no"));
        assert!(!confirmation_is_yes("delete"));
    }
}
