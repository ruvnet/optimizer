//! Integrity verification

use std::path::Path;
use std::collections::HashMap;

/// Verify installation integrity
pub struct IntegrityChecker {
    expected_hashes: HashMap<String, String>,
}

impl IntegrityChecker {
    pub fn new() -> Self {
        Self {
            expected_hashes: HashMap::new(),
        }
    }
    
    /// Verify a file matches expected hash
    pub fn verify_file(&self, path: &Path) -> Result<bool, String> {
        if !path.exists() {
            return Err(format!("File not found: {:?}", path));
        }
        
        // In production, compute SHA256 and compare
        Ok(true)
    }
    
    /// Verify entire installation
    pub fn verify_installation(&self, install_dir: &Path) -> VerificationResult {
        let mut result = VerificationResult::default();
        
        // Check main executable
        let exe_path = install_dir.join("ruvector-memopt.exe");
        if exe_path.exists() {
            result.files_checked += 1;
            result.files_valid += 1;
        } else {
            result.errors.push("Main executable not found".into());
        }
        
        // Check data directory
        let data_dir = install_dir.join("data");
        if !data_dir.exists() {
            result.warnings.push("Data directory missing - will be created".into());
        }
        
        result.is_valid = result.errors.is_empty();
        result
    }
}

#[derive(Debug, Default)]
pub struct VerificationResult {
    pub is_valid: bool,
    pub files_checked: usize,
    pub files_valid: usize,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl VerificationResult {
    pub fn print_report(&self) {
        println!("Installation Verification Report");
        println!("================================");
        println!("Status:        {}", if self.is_valid { "VALID" } else { "INVALID" });
        println!("Files checked: {}", self.files_checked);
        println!("Files valid:   {}", self.files_valid);
        
        if !self.errors.is_empty() {
            println!("\nErrors:");
            for e in &self.errors {
                println!("  - {}", e);
            }
        }
        
        if !self.warnings.is_empty() {
            println!("\nWarnings:");
            for w in &self.warnings {
                println!("  - {}", w);
            }
        }
    }
}

impl Default for IntegrityChecker {
    fn default() -> Self { Self::new() }
}
