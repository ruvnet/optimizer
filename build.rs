//! Build script for Windows resources

fn main() {
    // Rebuild if resources change
    println!("cargo:rerun-if-changed=resources/");
    println!("cargo:rerun-if-changed=build.rs");

    // Only embed resources on Windows
    #[cfg(windows)]
    {
        // Check which binary is being built
        let target = std::env::var("CARGO_BIN_NAME").unwrap_or_default();

        if target == "ruvector-memopt-tray" {
            // Embed tray manifest (no console, no elevation)
            embed_resource::compile("resources/tray.rc", embed_resource::NONE);
        }
    }
}
