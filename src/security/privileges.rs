//! Privilege management

use tracing::info;

#[derive(Debug, Clone, PartialEq)]
pub enum PrivilegeLevel {
    /// Standard user - limited optimization
    Standard,
    /// Elevated admin - full optimization
    Admin,
    /// System service - background operation
    System,
}

pub struct PrivilegeManager {
    level: PrivilegeLevel,
}

impl PrivilegeManager {
    pub fn new() -> Self {
        let level = Self::detect_level();
        info!("Running with privilege level: {:?}", level);
        Self { level }
    }
    
    fn detect_level() -> PrivilegeLevel {
        #[cfg(windows)]
        {
            // Check if running as service
            if std::env::var("RUNNING_AS_SERVICE").is_ok() {
                return PrivilegeLevel::System;
            }
            
            // Check admin
            if is_elevated() {
                return PrivilegeLevel::Admin;
            }
        }
        
        PrivilegeLevel::Standard
    }
    
    pub fn level(&self) -> &PrivilegeLevel {
        &self.level
    }
    
    pub fn can_clear_standby(&self) -> bool {
        matches!(self.level, PrivilegeLevel::Admin | PrivilegeLevel::System)
    }
    
    pub fn can_trim_system_processes(&self) -> bool {
        matches!(self.level, PrivilegeLevel::System)
    }
    
    pub fn can_install_service(&self) -> bool {
        matches!(self.level, PrivilegeLevel::Admin)
    }
}

#[cfg(windows)]
fn is_elevated() -> bool {
    std::process::Command::new("net")
        .args(["session"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

impl Default for PrivilegeManager {
    fn default() -> Self { Self::new() }
}
