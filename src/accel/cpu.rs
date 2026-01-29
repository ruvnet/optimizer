//! CPU feature detection

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

/// CPU capabilities for optimization decisions
#[derive(Debug, Clone)]
pub struct CpuCapabilities {
    pub vendor: String,
    pub model: String,
    pub has_avx: bool,
    pub has_avx2: bool,
    pub has_avx512: bool,
    pub has_avx_vnni: bool,
    pub has_npu: bool,
    pub has_neon: bool,
    pub core_count: usize,
    pub cache_line_size: usize,
}

impl CpuCapabilities {
    pub fn detect() -> Self {
        let mut caps = Self {
            vendor: String::new(),
            model: String::new(),
            has_avx: false,
            has_avx2: false,
            has_avx512: false,
            has_avx_vnni: false,
            has_npu: false,
            has_neon: false,
            core_count: num_cpus::get(),
            cache_line_size: 64,
        };

        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("avx") { caps.has_avx = true; }
            if is_x86_feature_detected!("avx2") { caps.has_avx2 = true; }
            if is_x86_feature_detected!("avx512f") { caps.has_avx512 = true; }

            // AVX-VNNI detection
            caps.has_avx_vnni = Self::detect_avx_vnni();

            // NPU detection (Intel Core Ultra)
            caps.has_npu = Self::detect_intel_npu();
        }

        #[cfg(target_arch = "aarch64")]
        {
            // ARM64 always has NEON
            caps.has_neon = true;
            caps.cache_line_size = 128; // Apple Silicon uses 128-byte cache lines
        }

        // Always detect vendor/model (works on all platforms)
        caps.vendor = Self::get_vendor();
        caps.model = Self::get_model();

        caps
    }

    #[cfg(target_arch = "x86_64")]
    fn detect_avx_vnni() -> bool {
        // CPUID leaf 7, subleaf 1, EAX bit 4
        unsafe {
            let result = __cpuid_count(7, 1);
            (result.eax & (1 << 4)) != 0
        }
    }

    #[cfg(not(target_arch = "x86_64"))]
    fn detect_avx_vnni() -> bool { false }

    #[cfg(target_arch = "x86_64")]
    fn detect_intel_npu() -> bool {
        // Intel NPU present in Core Ultra (Meteor Lake+)
        let model = Self::get_model();
        model.contains("Ultra") || model.contains("Meteor") || model.contains("Arrow")
    }

    #[cfg(not(target_arch = "x86_64"))]
    fn detect_intel_npu() -> bool { false }

    #[cfg(target_arch = "x86_64")]
    fn get_vendor() -> String {
        unsafe {
            let result = __cpuid(0);
            let vendor_bytes: [u8; 12] = std::mem::transmute([result.ebx, result.edx, result.ecx]);
            String::from_utf8_lossy(&vendor_bytes).trim().to_string()
        }
    }

    #[cfg(not(target_arch = "x86_64"))]
    fn get_vendor() -> String {
        #[cfg(target_os = "macos")]
        {
            if let Ok(output) = std::process::Command::new("sysctl")
                .args(["-n", "machdep.cpu.brand_string"])
                .output()
            {
                let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if s.contains("Apple") { return "Apple".into(); }
                if !s.is_empty() { return s.split_whitespace().next().unwrap_or("Unknown").into(); }
            }
        }
        "Unknown".into()
    }

    #[cfg(target_arch = "x86_64")]
    fn get_model() -> String {
        let mut model = String::new();
        unsafe {
            for i in 0x80000002u32..=0x80000004u32 {
                let result = __cpuid(i);
                let bytes: [u8; 16] = std::mem::transmute([result.eax, result.ebx, result.ecx, result.edx]);
                model.push_str(&String::from_utf8_lossy(&bytes));
            }
        }
        model.trim().to_string()
    }

    #[cfg(not(target_arch = "x86_64"))]
    fn get_model() -> String {
        #[cfg(target_os = "macos")]
        {
            if let Ok(output) = std::process::Command::new("sysctl")
                .args(["-n", "machdep.cpu.brand_string"])
                .output()
            {
                let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !s.is_empty() { return s; }
            }
            // Fallback: hw.model for Apple Silicon
            if let Ok(output) = std::process::Command::new("sysctl")
                .args(["-n", "hw.model"])
                .output()
            {
                let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !s.is_empty() { return format!("Apple {}", s); }
            }
        }
        "Unknown".into()
    }

    /// Get recommended SIMD width for operations
    pub fn recommended_simd_width(&self) -> usize {
        if self.has_avx512 { 512 }
        else if self.has_avx2 { 256 }
        else if self.has_avx { 128 }
        else { 64 }
    }

    /// Estimate speedup factor vs scalar
    pub fn estimated_speedup(&self) -> f32 {
        if self.has_avx512 { 16.0 }
        else if self.has_avx2 { 8.0 }
        else if self.has_avx { 4.0 }
        else if self.has_neon { 4.0 }  // ARM NEON is 128-bit (4x f32)
        else { 1.0 }
    }

    pub fn print_report(&self) {
        println!("CPU Capabilities:");
        println!("  Vendor:     {}", if self.vendor.is_empty() { "Unknown" } else { &self.vendor });
        println!("  Model:      {}", if self.model.is_empty() { "Unknown" } else { &self.model });
        println!("  Cores:      {}", self.core_count);
        if cfg!(target_arch = "aarch64") {
            println!("  NEON:       {}", if self.has_neon { "Yes (128-bit SIMD)" } else { "No" });
            println!("  Cache Line: {} bytes", self.cache_line_size);
        } else {
            println!("  AVX:        {}", if self.has_avx { "Yes" } else { "No" });
            println!("  AVX2:       {}", if self.has_avx2 { "Yes (8x SIMD)" } else { "No" });
            println!("  AVX-512:    {}", if self.has_avx512 { "Yes (16x SIMD)" } else { "No" });
            println!("  AVX-VNNI:   {}", if self.has_avx_vnni { "Yes (AI Accel)" } else { "No" });
            println!("  Intel NPU:  {}", if self.has_npu { "Yes (Neural Proc)" } else { "No" });
        }
        println!("  Est Speedup: {:.0}x", self.estimated_speedup());
    }
}

impl Default for CpuCapabilities {
    fn default() -> Self { Self::detect() }
}
