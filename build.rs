//! Build script for Windows resources

fn main() {
    // Rebuild if resources change
    println!("cargo:rerun-if-changed=resources/");
    println!("cargo:rerun-if-changed=build.rs");
    
    // Note: Manifest embedding removed to avoid conflicts
    // The manifest will be embedded via the installer instead
}
