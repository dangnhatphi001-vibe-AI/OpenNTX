mod app;
mod ui;

fn main() {
    if let Err(error) = app::AppPortalApp::from_env().and_then(|mut app| app.run()) {
        eprintln!("OpenNTX AppPortal error: {error}");
        std::process::exit(1);
    }
}
