//! Input validation and sanitization

use std::path::Path;

/// Validate process ID is reasonable
pub fn validate_pid(pid: u32) -> bool {
    pid > 0 && pid < 0xFFFFFFFF
}

/// Validate path is safe (no traversal attacks)
pub fn validate_path(path: &Path) -> bool {
    let path_str = path.to_string_lossy();
    
    // No path traversal
    if path_str.contains("..") {
        return false;
    }
    
    // No suspicious characters
    if path_str.contains('\0') || path_str.contains('|') || path_str.contains('<') || path_str.contains('>') {
        return false;
    }
    
    true
}

/// Validate configuration values
pub fn validate_config_value(key: &str, value: &str) -> bool {
    match key {
        "pressure_threshold" | "critical_threshold" => {
            value.parse::<u32>().map(|v| v <= 100).unwrap_or(false)
        }
        "min_interval_secs" => {
            value.parse::<u64>().map(|v| v >= 10 && v <= 3600).unwrap_or(false)
        }
        "ewc_lambda" => {
            value.parse::<f32>().map(|v| v >= 0.0 && v <= 1.0).unwrap_or(false)
        }
        _ => true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_pid_validation() {
        assert!(validate_pid(1234));
        assert!(!validate_pid(0));
    }
    
    #[test]
    fn test_path_validation() {
        assert!(validate_path(Path::new("C:/safe/path")));
        assert!(!validate_path(Path::new("../../../etc/passwd")));
    }
}
