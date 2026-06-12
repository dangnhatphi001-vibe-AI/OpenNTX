use crate::bridge::cli_bridge::CliBridge;
use crate::ui::{app_library, drop_zone, home, install_wizard, settings};
use openntx_core::registry::AppRegistry;

#[derive(Debug, Default)]
pub struct AppPortalApp {
    bridge: CliBridge,
}

impl AppPortalApp {
    pub fn run(&self) {
        println!("{}", home::render());
        println!();
        println!("{}", drop_zone::render());
        println!();
        println!("{}", install_wizard::render());
        println!();
        let registered_apps = AppRegistry::from_env()
            .and_then(|registry| registry.list_apps())
            .unwrap_or_default();
        println!("{}", app_library::render(&registered_apps));
        println!();
        println!("{}", settings::render());
        println!();
        println!("CLI bridge preview:");
        for command in self
            .bridge
            .preview_install_commands("~/Downloads/setup.exe")
        {
            println!("  {command}");
        }
    }
}
