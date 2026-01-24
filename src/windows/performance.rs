//! Windows performance optimization utilities

use std::process::Command;
use std::path::PathBuf;

/// Clear temporary files
pub fn cleanup_temp_files() -> Result<String, String> {
    let temp_dirs = [
        std::env::var("TEMP").unwrap_or_default(),
        std::env::var("TMP").unwrap_or_default(),
    ];
    
    let mut total_freed = 0u64;
    let mut files_deleted = 0u32;
    
    for dir in &temp_dirs {
        if dir.is_empty() { continue; }
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                if let Ok(metadata) = entry.metadata() {
                    let size = metadata.len();
                    if std::fs::remove_file(entry.path()).is_ok() ||
                       std::fs::remove_dir_all(entry.path()).is_ok() {
                        total_freed += size;
                        files_deleted += 1;
                    }
                }
            }
        }
    }
    
    Ok(format!("Deleted {} items\nFreed: {:.1} MB", 
        files_deleted, total_freed as f64 / 1024.0 / 1024.0))
}

/// Flush DNS cache
pub fn flush_dns() -> Result<String, String> {
    let output = Command::new("ipconfig")
        .args(["/flushdns"])
        .output()
        .map_err(|e| e.to_string())?;
    
    if output.status.success() {
        Ok("DNS cache flushed".to_string())
    } else {
        Err("Failed to flush DNS".to_string())
    }
}

/// Set high performance power plan
pub fn set_high_performance() -> Result<String, String> {
    let output = Command::new("powercfg")
        .args(["/setactive", "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c"])
        .output()
        .map_err(|e| e.to_string())?;
    
    if output.status.success() {
        Ok("High Performance mode activated".to_string())
    } else {
        Err("Need admin rights for power plan".to_string())
    }
}

/// Clear thumbnail cache
pub fn cleanup_thumbnails() -> Result<String, String> {
    let mut thumb_path = PathBuf::from(std::env::var("USERPROFILE").unwrap_or_default());
    thumb_path.push("AppData");
    thumb_path.push("Local");
    thumb_path.push("Microsoft");
    thumb_path.push("Windows");
    thumb_path.push("Explorer");
    
    let mut deleted = 0u32;
    
    if let Ok(entries) = std::fs::read_dir(&thumb_path) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("thumbcache_") {
                if std::fs::remove_file(entry.path()).is_ok() {
                    deleted += 1;
                }
            }
        }
    }
    
    Ok(format!("Cleared {} thumbnail caches", deleted))
}
