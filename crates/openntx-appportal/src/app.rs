use crate::ui::{app_library, home, install_wizard, settings};
use openntx_core::app_id::generate_app_id;
use openntx_core::capture::CaptureRegistryService;
use openntx_core::doctor::{app_doctor, global_doctor, repair_app};
use openntx_core::logs::{list_logs, show_log};
use openntx_core::manifest::{
    generate_manifest_from_pe, AppManifest, GeneratedManifest, ManifestGenerationInput,
};
use openntx_core::packaging::{build_deb_package, DebBuildOptions};
use openntx_core::pe::{analyze_pe, PeAnalysis};
use openntx_core::registry::{
    AppRegistry, DesktopMode, InstallPlan, RegisteredApp, RegistrationResult, RemoveMode,
};
use openntx_core::runtime::{create_registered_run_plan, RunPlanOptions};
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
                "c" | "4" => self.capture_screen()?,
                "p" | "5" => self.packaging_screen()?,
                "lg" | "6" => self.logs_screen()?,
                "d" | "7" => self.doctor_screen()?,
                "s" | "8" => self.settings_screen()?,
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
                value if value.eq_ignore_ascii_case("r") => {
                    self.run_plan_screen(&manifest.app_id)?
                }
                value if value.eq_ignore_ascii_case("c") => self.create_desktop_screen(app_id)?,
                value if value.eq_ignore_ascii_case("x") => self.remove_desktop_screen(app_id)?,
                value if value.eq_ignore_ascii_case("1") => {
                    self.capture_snapshot_before_screen(app_id)?
                }
                value if value.eq_ignore_ascii_case("2") => {
                    self.capture_snapshot_after_screen(app_id)?
                }
                value if value.eq_ignore_ascii_case("3") => self.capture_diff_screen(app_id)?,
                value if value.eq_ignore_ascii_case("4") => self.capture_report_screen(app_id)?,
                value if value.eq_ignore_ascii_case("5") => self.capture_status_screen(app_id)?,
                value if value.eq_ignore_ascii_case("p") => self.package_screen(app_id)?,
                value if value.eq_ignore_ascii_case("l") => self.show_app_log_screen(app_id)?,
                value if value.eq_ignore_ascii_case("dd") => {
                    let report = app_doctor(&self.registry, app_id)?;
                    clear_screen();
                    println!("Doctor: {app_id}");
                    println!("Status: {}", report.status);
                    for w in &report.warnings {
                        println!("  ! {w}");
                    }
                    pause("Press Enter.")?;
                }
                value if value.eq_ignore_ascii_case("dr") => {
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

    fn settings_screen(&self) -> Result<()> {
        clear_screen();
        println!("{}", settings::render(self.registry.paths()));
        pause("Press Enter to return.")
    }

    fn capture_screen(&mut self) -> Result<()> {
        loop {
            let apps = self.registry.list_apps()?;
            clear_screen();
            println!("OpenNTX Capture");
            println!("---------------");
            println!("Installer execution is not implemented. Capture snapshots only inspect OpenNTX-managed app directories.");
            println!();
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
            loop {
                clear_screen();
                println!("OpenNTX Capture for {}", app_id);
                println!("--------------------------------");
                println!("[1] Snapshot Before");
                println!("[2] Snapshot After");
                println!("[3] Diff");
                println!("[4] Report");
                println!("[5] Status");
                println!("[B] Back");
                match prompt("Select action")?.as_str() {
                    value if value.eq_ignore_ascii_case("1") => {
                        self.capture_snapshot_before_screen(&app_id)?
                    }
                    value if value.eq_ignore_ascii_case("2") => {
                        self.capture_snapshot_after_screen(&app_id)?
                    }
                    value if value.eq_ignore_ascii_case("3") => {
                        self.capture_diff_screen(&app_id)?
                    }
                    value if value.eq_ignore_ascii_case("4") => {
                        self.capture_report_screen(&app_id)?
                    }
                    value if value.eq_ignore_ascii_case("5") => {
                        self.capture_status_screen(&app_id)?
                    }
                    value if value.eq_ignore_ascii_case("b") => break,
                    "" => {}
                    _ => pause("Unknown capture action.")?,
                }
            }
        }
    }

    fn capture_snapshot_before_screen(&self, app_id: &str) -> Result<()> {
        let service = CaptureRegistryService::new(AppRegistry::new(self.registry.paths().clone()));
        let result = match service.snapshot_before(app_id) {
            Ok(r) => r,
            Err(e) => {
                pause(&format!("Snapshot failed: {e}"))?;
                return Ok(());
            }
        };

        clear_screen();
        println!("OpenNTX Capture Snapshot Before");
        println!("-------------------------------");
        println!("App ID: {app_id}");
        println!("Snapshot path: {}", result.snapshot_path.display());
        println!("Entries: {}", result.snapshot.entries.len());
        println!("Errors: {}", result.snapshot.errors.len());
        println!();
        println!("Capture snapshots only inspect OpenNTX-managed app directories. No installer is executed.");
        pause("Snapshot-before written.")
    }

    fn capture_snapshot_after_screen(&self, app_id: &str) -> Result<()> {
        let service = CaptureRegistryService::new(AppRegistry::new(self.registry.paths().clone()));
        let result = match service.snapshot_after(app_id) {
            Ok(r) => r,
            Err(e) => {
                pause(&format!("Snapshot failed: {e}"))?;
                return Ok(());
            }
        };

        clear_screen();
        println!("OpenNTX Capture Snapshot After");
        println!("------------------------------");
        println!("App ID: {app_id}");
        println!("Snapshot path: {}", result.snapshot_path.display());
        println!("Entries: {}", result.snapshot.entries.len());
        println!("Errors: {}", result.snapshot.errors.len());
        println!();
        println!("Capture snapshots only inspect OpenNTX-managed app directories. No installer is executed.");
        pause("Snapshot-after written.")
    }

    fn capture_diff_screen(&self, app_id: &str) -> Result<()> {
        let service = CaptureRegistryService::new(AppRegistry::new(self.registry.paths().clone()));
        let result = match service.diff(app_id) {
            Ok(r) => r,
            Err(e) => {
                pause(&format!("Diff failed: {e}"))?;
                return Ok(());
            }
        };

        clear_screen();
        println!("OpenNTX Capture Diff");
        println!("--------------------");
        println!("App ID: {app_id}");
        println!("Diff path: {}", result.diff_path.display());
        println!();
        println!("Files created: {}", result.diff.files_created.len());
        println!("Files removed: {}", result.diff.files_removed.len());
        println!("Files modified: {}", result.diff.files_modified.len());
        println!(
            "Directories created: {}",
            result.diff.directories_created.len()
        );
        println!(
            "Directories removed: {}",
            result.diff.directories_removed.len()
        );
        println!("Symlinks created: {}", result.diff.symlinks_created.len());
        println!("Symlinks removed: {}", result.diff.symlinks_removed.len());
        println!(
            "Registry files changed: {}",
            result.diff.registry_files_changed.len()
        );
        pause("Diff written.")
    }

    fn capture_report_screen(&self, app_id: &str) -> Result<()> {
        let service = CaptureRegistryService::new(AppRegistry::new(self.registry.paths().clone()));
        let result = match service.report(app_id) {
            Ok(r) => r,
            Err(e) => {
                pause(&format!("Report failed: {e}"))?;
                return Ok(());
            }
        };

        clear_screen();
        println!("OpenNTX Capture Report");
        println!("----------------------");
        println!("App ID: {app_id}");
        println!("App name: {}", result.report.app_name);
        println!("Report path: {}", result.report_path.display());
        println!();
        println!("Files created: {}", result.report.files_created.len());
        println!("Files modified: {}", result.report.files_modified.len());
        println!("Files removed: {}", result.report.files_removed.len());
        println!(
            "Registry files changed: {}",
            result.diff.registry_files_changed.len()
        );
        println!("Status: {}", result.report.status);
        pause("Report written.")
    }

    fn capture_status_screen(&self, app_id: &str) -> Result<()> {
        let service = CaptureRegistryService::new(AppRegistry::new(self.registry.paths().clone()));
        let status = match service.status(app_id) {
            Ok(s) => s,
            Err(e) => {
                pause(&format!("Status failed: {e}"))?;
                return Ok(());
            }
        };

        clear_screen();
        println!("OpenNTX Capture Status");
        println!("----------------------");
        println!("App ID: {}", status.app_id);
        println!("App name: {}", status.app_name);
        println!("App directory: {}", status.app_dir.display());
        println!("Capture directory: {}", status.capture_dir.display());
        println!();
        println!(
            "Snapshot before: {}",
            if status.snapshot_before {
                "present"
            } else {
                "missing"
            }
        );
        println!(
            "Snapshot after: {}",
            if status.snapshot_after {
                "present"
            } else {
                "missing"
            }
        );
        println!("Diff: {}", if status.diff { "present" } else { "missing" });
        println!(
            "Report: {}",
            if status.report { "present" } else { "missing" }
        );
        println!();
        println!("Capture snapshots only inspect OpenNTX-managed app directories. No installer is executed.");
        pause("Press Enter to return.")
    }

    fn package_screen(&self, app_id: &str) -> Result<()> {
        let manifest = self.registry.load_manifest(app_id)?;

        // Show dry-run plan first
        let mut options = DebBuildOptions::new(app_id);
        options.dry_run = true;

        let plan = match build_deb_package(&self.registry, &options) {
            Ok(p) => p,
            Err(e) => {
                pause(&format!("Package plan failed: {e}"))?;
                return Ok(());
            }
        };

        clear_screen();
        println!("OpenNTX Package (.deb)");
        println!("----------------------");
        println!("App ID: {app_id}");
        println!("App name: {}", manifest.name);
        println!("Package name: {}", plan.package_name);
        println!("Version: {}", plan.version);
        println!("Deb filename: {}", plan.deb_filename);
        println!();
        println!("Files to package:");
        for file in &plan.files_to_package {
            println!("  {file}");
        }
        println!();
        println!("Layout app root: {}", plan.layout.app_root.display());
        println!(
            "Layout desktop entry: {}",
            plan.layout.desktop_entry_path.display()
        );
        println!("Runtime dependency: {}", plan.layout.runtime_dependency);
        println!();
        println!("No EXE files will be executed. No installers will run.");
        println!();

        if confirm("Build this .deb package?")? {
            let mut build_options = DebBuildOptions::new(app_id);
            build_options.dry_run = false;

            match build_deb_package(&self.registry, &build_options) {
                Ok(built_plan) => {
                    pause(&format!(
                        "Package built: {}",
                        built_plan
                            .output_dir
                            .join(&built_plan.deb_filename)
                            .display()
                    ))?;
                }
                Err(e) => {
                    pause(&format!("Package build failed: {e}"))?;
                }
            }
        } else {
            pause("Package not built.")?;
        }
        Ok(())
    }

    fn run_plan_screen(&self, app_id: &str) -> Result<()> {
        let report =
            create_registered_run_plan(&self.registry, app_id, &RunPlanOptions::default())?;
        clear_screen();
        println!("OpenNTX Run Plan");
        println!("----------------");
        println!("Target: {}", report.target);
        println!("App ID: {}", report.app_id);
        println!("Name: {}", report.name);
        println!("Executable: {}", report.executable_path);
        println!("Architecture: {}", report.architecture);
        println!("Install mode: {}", report.install_mode);
        println!("Sandbox profile: {}", report.sandbox_profile);
        println!("Imported DLL count: {}", report.imported_dll_count);
        println!("Desktop status: {}", report.desktop_status);
        println!("Backend: {}", report.backend);
        println!("Status: {}", report.status);
        println!("Timestamp: {}", report.timestamp);
        if let Some(log_path) = &report.log_path {
            println!("Run-plan log: {log_path}");
        }
        println!();
        println!(
            "{} is registered, but runtime execution is not implemented in V0.8.",
            report.name
        );
        println!("This action only validates app metadata and prepares a future execution plan.");
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

    fn show_app_log_screen(&self, app_id: &str) -> Result<()> {
        match show_log(&self.registry, app_id) {
            Ok(report) => {
                clear_screen();
                println!("Run-Plan Log for {app_id}");
                println!("-------------------------");
                println!("Name: {}", report.name);
                println!("Timestamp: {}", report.timestamp);
                println!("Executable: {}", report.executable_path);
                println!("Architecture: {}", report.architecture);
                println!("Status: {}", report.status);
                println!("Backend: {}", report.backend);
                if let Some(log_path) = &report.log_path {
                    println!("Log path: {log_path}");
                }
                println!();
                println!("{}", report.message);
            }
            Err(_) => {
                clear_screen();
                println!("No run-plan log found for {app_id}.");
            }
        }
        pause("Press Enter to return.")
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

    fn doctor_screen(&self) -> Result<()> {
        loop {
            clear_screen();
            println!("OpenNTX Doctor");
            println!("--------------");
            println!("[1] Global diagnosis");
            println!("[2] App diagnosis");
            println!("[B] Back");
            match prompt("Select action")?.as_str() {
                value if value.eq_ignore_ascii_case("1") => self.doctor_global_screen()?,
                value if value.eq_ignore_ascii_case("2") => self.doctor_app_screen()?,
                value if value.eq_ignore_ascii_case("b") => return Ok(()),
                "" => {}
                _ => pause("Unknown action.")?,
            }
        }
    }

    fn doctor_global_screen(&self) -> Result<()> {
        let report = global_doctor(&self.registry)?;
        clear_screen();
        println!("OpenNTX Doctor - Global");
        println!("-----------------------");
        println!(
            "Data directory: {} ({})",
            report.data_dir_path,
            if report.data_dir_exists {
                "exists"
            } else {
                "missing"
            }
        );
        println!("Registered apps: {}", report.app_count);
        println!("Broken apps: {}", report.broken_app_count);
        if !report.broken_apps.is_empty() {
            for app_id in &report.broken_apps {
                println!("  Broken: {app_id}");
            }
        }
        println!(
            "Logs directory: {} ({}, writable={})",
            report.logs_dir_path,
            if report.logs_dir_exists {
                "exists"
            } else {
                "missing"
            },
            report.logs_dir_writable
        );
        println!(
            "Desktop entries: {}",
            if report.desktop_entries_dir_exists {
                "exists"
            } else {
                "missing"
            }
        );
        println!(
            "dpkg-deb: {}",
            if report.dpkg_deb_available {
                "available"
            } else {
                "not available"
            }
        );
        println!(
            "notify-send: {}",
            if report.notify_send_available {
                "available"
            } else {
                "not available"
            }
        );
        if !report.warnings.is_empty() {
            println!();
            println!("Warnings:");
            for w in &report.warnings {
                println!("  ! {w}");
            }
        }
        println!();
        println!("Status: {}", report.status);
        pause("Press Enter to return.")
    }

    fn doctor_app_screen(&self) -> Result<()> {
        let apps = self.registry.list_apps()?;
        if apps.is_empty() {
            clear_screen();
            println!("No registered apps.");
            pause("Press Enter to return.")?;
            return Ok(());
        }
        clear_screen();
        println!("OpenNTX Doctor - Select App");
        println!("---------------------------");
        print_numbered_apps(&apps);
        let input = prompt("Select app number or [B] Back")?;
        if input.eq_ignore_ascii_case("b") {
            return Ok(());
        }
        let Some(index) = parse_menu_index(&input, apps.len()) else {
            pause("Invalid selection.")?;
            return Ok(());
        };
        let app_id = apps[index].app_id.clone();

        let report = app_doctor(&self.registry, &app_id)?;
        clear_screen();
        println!("OpenNTX Doctor - App: {app_id}");
        println!("--------------------------------");
        println!("App name: {}", report.app_name);
        println!("Manifest exists: {}", report.manifest_exists);
        println!("Manifest regular file: {}", report.manifest_is_regular_file);
        println!("Manifest valid: {}", report.manifest_valid);
        if let Some(err) = &report.manifest_validation_error {
            if !report.manifest_valid {
                println!("Validation error: {err}");
            }
        }
        println!(
            "Install plan: {}",
            if report.install_plan_exists {
                "exists"
            } else {
                "missing"
            }
        );
        println!(
            "drive_c: {} ({})",
            if report.drive_c_exists {
                "exists"
            } else {
                "missing"
            },
            if report.drive_c_is_real_dir {
                "real dir"
            } else {
                "not real dir"
            }
        );
        println!(
            "registry: {} ({})",
            if report.registry_exists {
                "exists"
            } else {
                "missing"
            },
            if report.registry_is_real_dir {
                "real dir"
            } else {
                "not real dir"
            }
        );
        println!(
            "Capture dir: {}",
            if report.capture_dir_exists {
                "exists"
            } else {
                "missing"
            }
        );
        println!(
            "Desktop entry: {}",
            if report.desktop_entry_exists {
                "present"
            } else {
                "missing"
            }
        );
        println!("Package build possible: {}", report.package_build_possible);
        println!("Log dir writable: {}", report.log_dir_writable);
        if !report.unsafe_symlinks.is_empty() {
            println!();
            println!("Unsafe symlinks:");
            for link in &report.unsafe_symlinks {
                println!("  !! {link}");
            }
        }
        if !report.warnings.is_empty() {
            println!();
            println!("Warnings:");
            for w in &report.warnings {
                println!("  ! {w}");
            }
        }
        println!();
        println!("Status: {}", report.status);

        println!();
        println!("[R] Repair (dry-run)");
        println!("[B] Back");
        match prompt("Select action")?.as_str() {
            value if value.eq_ignore_ascii_case("r") => {
                let plan = repair_app(&self.registry, &app_id, true)?;
                clear_screen();
                println!("OpenNTX Doctor Repair (Dry-Run)");
                println!("-------------------------------");
                if !plan.unsafe_symlinks_found.is_empty() {
                    println!("UNSAFE SYMLINKS FOUND - REPAIR REFUSED:");
                    for link in &plan.unsafe_symlinks_found {
                        println!("  !! {link}");
                    }
                } else if plan.actions.is_empty() {
                    println!("No repairs needed. App is healthy.");
                } else {
                    for action in &plan.actions {
                        println!("  [planned] {} -> {}", action.description, action.path);
                    }
                }
                pause("Dry-run complete.")?;
            }
            _ => {}
        }
        Ok(())
    }

    fn logs_screen(&self) -> Result<()> {
        loop {
            let logs = list_logs(&self.registry)?;
            clear_screen();
            println!("OpenNTX Logs");
            println!("------------");
            println!("Total logs: {}", logs.len());
            if logs.is_empty() {
                println!("No run-plan logs found.");
                println!();
                println!("[B] Back");
                match prompt("Select action")?.as_str() {
                    value if value.eq_ignore_ascii_case("b") => return Ok(()),
                    "" => {}
                    _ => pause("Unknown action.")?,
                }
                continue;
            }
            println!();
            for (i, log) in logs.iter().take(20).enumerate() {
                println!(
                    "[{}] {} | {} | {} | {}",
                    i + 1,
                    log.timestamp,
                    log.app_id,
                    log.app_name,
                    log.status
                );
            }
            println!();
            println!("[1-20] Show log details");
            println!("[B] Back");
            let input = prompt("Select action")?;
            if input.eq_ignore_ascii_case("b") {
                return Ok(());
            }
            if let Some(index) = parse_menu_index(&input, logs.len().min(20)) {
                match show_log(&self.registry, &logs[index].file_path) {
                    Ok(report) => {
                        clear_screen();
                        println!("Run-Plan Log Details");
                        println!("--------------------");
                        println!("App ID: {}", report.app_id);
                        println!("Name: {}", report.name);
                        println!("Target: {}", report.target);
                        println!("Timestamp: {}", report.timestamp);
                        println!("Executable: {}", report.executable_path);
                        println!("Architecture: {}", report.architecture);
                        println!("Status: {}", report.status);
                        println!("Backend: {}", report.backend);
                        if let Some(log_path) = &report.log_path {
                            println!("Log path: {log_path}");
                        }
                        println!();
                        println!("{}", report.message);
                        pause("Press Enter.")?;
                    }
                    Err(e) => pause(&format!("Error: {e}"))?,
                }
            }
        }
    }

    fn packaging_screen(&self) -> Result<()> {
        let apps = self.registry.list_apps()?;
        if apps.is_empty() {
            clear_screen();
            println!("No registered apps.");
            pause("Press Enter to return.")?;
            return Ok(());
        }
        loop {
            clear_screen();
            println!("OpenNTX Package Builder");
            println!("-----------------------");
            print_numbered_apps(&apps);
            println!();
            println!("[B] Back");
            let input = prompt("Select app number or [B] Back")?;
            if input.eq_ignore_ascii_case("b") {
                return Ok(());
            }
            let Some(index) = parse_menu_index(&input, apps.len()) else {
                pause("Invalid selection.")?;
                continue;
            };
            let app_id = apps[index].app_id.clone();

            // Show dry-run plan first
            let mut options = DebBuildOptions::new(&app_id);
            options.dry_run = true;

            let plan = match build_deb_package(&self.registry, &options) {
                Ok(p) => p,
                Err(e) => {
                    pause(&format!("Package plan failed: {e}"))?;
                    continue;
                }
            };

            clear_screen();
            println!("OpenNTX Package Plan");
            println!("--------------------");
            println!("App ID: {app_id}");
            println!("Package: {}", plan.package_name);
            println!("Version: {}", plan.version);
            println!("Deb file: {}", plan.deb_filename);
            println!();
            println!("Files:");
            for f in &plan.files_to_package {
                println!("  {f}");
            }
            println!();
            println!("[B] Build .deb (with confirmation)");
            println!("[R] Return");
            match prompt("Select action")?.as_str() {
                value if value.eq_ignore_ascii_case("b") => {
                    if confirm("Build this .deb package?")? {
                        let mut build_options = DebBuildOptions::new(&app_id);
                        build_options.dry_run = false;
                        match build_deb_package(&self.registry, &build_options) {
                            Ok(built) => {
                                pause(&format!(
                                    "Package built: {}",
                                    built.output_dir.join(&built.deb_filename).display()
                                ))?;
                            }
                            Err(e) => pause(&format!("Build failed: {e}"))?,
                        }
                    } else {
                        pause("Not built.")?;
                    }
                }
                _ => {}
            }
        }
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
    println!("[1] Capture: Snapshot Before");
    println!("[2] Capture: Snapshot After");
    println!("[3] Capture: Diff");
    println!("[4] Capture: Report");
    println!("[5] Capture: Status");
    println!("[P] Package (.deb)");
    println!("[L] Show logs");
    println!("[DD] Doctor");
    println!("[DR] Dry-run remove app");
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
