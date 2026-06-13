// build.rs — Compile Slint UI files at build time.
//
// This script uses `slint-build` to compile `.slint` files in the `ui/`
// directory into Rust code that is included in the binary via
// `slint::include_modules!()`.

fn main() {
    slint_build::compile("ui/app_window.slint").expect("failed to compile Slint UI");
}
