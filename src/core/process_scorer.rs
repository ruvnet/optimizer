//! Process scoring for intelligent trimming decisions

use sysinfo::{System, ProcessesToUpdate};
use std::collections::HashMap;

pub struct ProcessScorer {
    system: System,
    priorities: HashMap<String, u32>,
}

impl ProcessScorer {
    pub fn new() -> Self {
        let mut priorities = HashMap::new();
        for proc in ["System", "csrss.exe", "smss.exe", "lsass.exe", "services.exe"] {
            priorities.insert(proc.to_lowercase(), 0);
        }
        for proc in ["explorer.exe", "dwm.exe", "svchost.exe"] {
            priorities.insert(proc.to_lowercase(), 10);
        }
        for proc in ["chrome.exe", "firefox.exe", "msedge.exe", "code.exe", "slack.exe"] {
            priorities.insert(proc.to_lowercase(), 50);
        }
        Self { system: System::new_all(), priorities }
    }

    pub fn refresh(&mut self) {
        self.system.refresh_processes(ProcessesToUpdate::All, true);
    }

    pub fn get_trim_candidates(&self, limit: usize) -> Vec<u32> {
        let mut candidates: Vec<(u32, u64, u32)> = self.system
            .processes()
            .iter()
            .filter_map(|(pid, proc)| {
                let name = proc.name().to_string_lossy().to_lowercase();
                let priority = self.priorities.get(&name).copied().unwrap_or(30);
                if priority == 0 { return None; }
                Some((pid.as_u32(), proc.memory(), priority))
            })
            .collect();
        candidates.sort_by(|a, b| ((b.2 as u64) * b.1).cmp(&((a.2 as u64) * a.1)));
        candidates.into_iter().take(limit).map(|(pid, _, _)| pid).collect()
    }

    pub fn get_memory_by_name(&self, name: &str) -> u64 {
        let name_lower = name.to_lowercase();
        self.system.processes().values()
            .filter(|p| p.name().to_string_lossy().to_lowercase().contains(&name_lower))
            .map(|p| p.memory())
            .sum()
    }

    pub fn process_count(&self) -> usize { self.system.processes().len() }
}

impl Default for ProcessScorer {
    fn default() -> Self { Self::new() }
}
