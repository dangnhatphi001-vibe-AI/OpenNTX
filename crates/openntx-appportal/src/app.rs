use crate::bridge::cli_bridge::CliBridge;
use crate::ui::{app_library, drop_zone, home, install_wizard, settings};

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
        println!("{}", app_library::render());
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
