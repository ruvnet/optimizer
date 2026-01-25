//! Build script for platform-specific resources
//!
//! On Windows: Embeds manifest and icon resources
//! On Linux: No-op (resources not needed)

fn main() {
    // Rebuild if build script changes
    println!("cargo:rerun-if-changed=build.rs");

    // Windows-specific resource embedding
    #[cfg(windows)]
    {
        // Only trigger rebuild if resources directory exists
        if std::path::Path::new("resources").exists() {
            println!("cargo:rerun-if-changed=resources/");
        }

        // Check which binary is being built
        let target = std::env::var("CARGO_BIN_NAME").unwrap_or_default();

        if target == "ruvector-memopt-tray" {
            // Embed tray manifest (no console, no elevation)
            if std::path::Path::new("resources/tray.rc").exists() {
                embed_resource::compile("resources/tray.rc", embed_resource::NONE);
            }
        }
    }

    // Linux-specific build steps (currently no-op)
    #[cfg(target_os = "linux")]
    {
        // Future: Could generate .desktop files, systemd service files, etc.
        // For now, just print build info
        println!("cargo:rustc-env=BUILD_PLATFORM=linux");
    }
}
