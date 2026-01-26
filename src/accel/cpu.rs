//! CPU feature detection for Intel and Apple Silicon Macs

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

/// CPU capabilities for optimization decisions
#[derive(Debug, Clone)]
pub struct CpuCapabilities {
    pub vendor: String,
    pub model: String,
    /// x86 AVX support (Intel/AMD)
    pub has_avx: bool,
    /// x86 AVX2 support (Intel/AMD)
    pub has_avx2: bool,
    /// x86 AVX-512 support (Intel/AMD)
    pub has_avx512: bool,
    /// x86 AVX-VNNI support (Intel)
    pub has_avx_vnni: bool,
    /// ARM NEON support (Apple Silicon)
    pub has_neon: bool,
    /// ARM AMX support (Apple M4+)
    pub has_amx: bool,
    /// Neural processing unit (Intel NPU or Apple Neural Engine)
    pub has_npu: bool,
    pub core_count: usize,
    /// Performance cores (Apple Silicon)
    pub perf_core_count: usize,
    /// Efficiency cores (Apple Silicon)
    pub efficiency_core_count: usize,
    pub cache_line_size: usize,
    /// Architecture type
    pub arch: CpuArch,
}

/// CPU architecture type
#[derive(Debug, Clone, PartialEq)]
pub enum CpuArch {
    X86_64,
    Arm64,
    Unknown,
}

impl CpuCapabilities {
    pub fn detect() -> Self {
        let core_count = num_cpus::get();

        let mut caps = Self {
            vendor: String::new(),
            model: String::new(),
            has_avx: false,
            has_avx2: false,
            has_avx512: false,
            has_avx_vnni: false,
            has_neon: false,
            has_amx: false,
            has_npu: false,
            core_count,
            perf_core_count: 0,
            efficiency_core_count: 0,
            cache_line_size: 64,
            arch: CpuArch::Unknown,
        };

        #[cfg(target_arch = "x86_64")]
        {
            caps.arch = CpuArch::X86_64;
            caps.cache_line_size = 64;

            if is_x86_feature_detected!("avx") { caps.has_avx = true; }
            if is_x86_feature_detected!("avx2") { caps.has_avx2 = true; }
            if is_x86_feature_detected!("avx512f") { caps.has_avx512 = true; }

            // AVX-VNNI detection
            caps.has_avx_vnni = Self::detect_avx_vnni();

            // NPU detection (Intel Core Ultra)
            caps.has_npu = Self::detect_intel_npu();
            caps.vendor = Self::get_x86_vendor();
            caps.model = Self::get_x86_model();
        }

        #[cfg(target_arch = "aarch64")]
        {
            caps.arch = CpuArch::Arm64;
            caps.cache_line_size = 128; // Apple Silicon uses 128-byte cache lines
            caps.has_neon = true; // All Apple Silicon has NEON

            // Detect Apple Silicon details
            caps.vendor = "Apple".to_string();
            caps.model = Self::get_apple_chip_model();

            // Apple Neural Engine is present on all Apple Silicon Macs
            caps.has_npu = true;

            // Detect M4+ AMX support based on model
            caps.has_amx = caps.model.contains("M4");

            // Get core counts
            let (perf, eff) = Self::get_apple_core_counts();
            caps.perf_core_count = perf;
            caps.efficiency_core_count = eff;
        }

        caps
    }

    // ========== x86_64 (Intel Mac) detection ==========

    #[cfg(target_arch = "x86_64")]
    fn detect_avx_vnni() -> bool {
        // CPUID leaf 7, subleaf 1, EAX bit 4
        unsafe {
            let result = __cpuid_count(7, 1);
            (result.eax & (1 << 4)) != 0
        }
    }

    #[cfg(target_arch = "x86_64")]
    fn detect_intel_npu() -> bool {
        // Intel NPU present in Core Ultra (Meteor Lake+)
        let model = Self::get_x86_model();
        model.contains("Ultra") || model.contains("Meteor") || model.contains("Arrow")
    }

    #[cfg(target_arch = "x86_64")]
    fn get_x86_vendor() -> String {
        unsafe {
            let result = __cpuid(0);
            let vendor_bytes: [u8; 12] = std::mem::transmute([result.ebx, result.edx, result.ecx]);
            String::from_utf8_lossy(&vendor_bytes).trim().to_string()
        }
    }

    #[cfg(target_arch = "x86_64")]
    fn get_x86_model() -> String {
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

    // ========== aarch64 (Apple Silicon) detection ==========

    #[cfg(target_arch = "aarch64")]
    fn get_apple_chip_model() -> String {
        // Use sysctl to get the chip model
        Self::sysctl_string("machdep.cpu.brand_string")
            .or_else(|| Self::sysctl_string("hw.model"))
            .unwrap_or_else(|| "Apple Silicon".to_string())
    }

    #[cfg(target_arch = "aarch64")]
    fn get_apple_core_counts() -> (usize, usize) {
        let perf = Self::sysctl_int("hw.perflevel0.physicalcpu").unwrap_or(0) as usize;
        let eff = Self::sysctl_int("hw.perflevel1.physicalcpu").unwrap_or(0) as usize;
        (perf, eff)
    }

    #[cfg(target_arch = "aarch64")]
    fn sysctl_string(name: &str) -> Option<String> {
        use std::ffi::CString;
        use std::ptr;

        let name_cstr = CString::new(name).ok()?;
        let mut size: libc::size_t = 0;

        // First call to get size
        unsafe {
            if libc::sysctlbyname(
                name_cstr.as_ptr(),
                ptr::null_mut(),
                &mut size,
                ptr::null_mut(),
                0,
            ) != 0 {
                return None;
            }
        }

        if size == 0 {
            return None;
        }

        // Second call to get value
        let mut buf = vec![0u8; size];
        unsafe {
            if libc::sysctlbyname(
                name_cstr.as_ptr(),
                buf.as_mut_ptr() as *mut libc::c_void,
                &mut size,
                ptr::null_mut(),
                0,
            ) != 0 {
                return None;
            }
        }

        // Remove null terminator and convert to string
        if let Some(pos) = buf.iter().position(|&b| b == 0) {
            buf.truncate(pos);
        }
        String::from_utf8(buf).ok()
    }

    #[cfg(target_arch = "aarch64")]
    fn sysctl_int(name: &str) -> Option<i64> {
        use std::ffi::CString;

        let name_cstr = CString::new(name).ok()?;
        let mut value: i64 = 0;
        let mut size: libc::size_t = std::mem::size_of::<i64>();

        unsafe {
            if libc::sysctlbyname(
                name_cstr.as_ptr(),
                &mut value as *mut i64 as *mut libc::c_void,
                &mut size,
                std::ptr::null_mut(),
                0,
            ) != 0 {
                return None;
            }
        }

        Some(value)
    }

    // ========== Common methods ==========

    /// Get recommended SIMD width for operations
    pub fn recommended_simd_width(&self) -> usize {
        match self.arch {
            CpuArch::X86_64 => {
                if self.has_avx512 { 512 }
                else if self.has_avx2 { 256 }
                else if self.has_avx { 128 }
                else { 64 }
            }
            CpuArch::Arm64 => {
                // NEON is 128-bit, AMX provides matrix acceleration
                if self.has_amx { 256 } // AMX can process wider
                else { 128 }
            }
            CpuArch::Unknown => 64,
        }
    }

    /// Estimate speedup factor vs scalar
    pub fn estimated_speedup(&self) -> f32 {
        match self.arch {
            CpuArch::X86_64 => {
                if self.has_avx512 { 16.0 }
                else if self.has_avx2 { 8.0 }
                else if self.has_avx { 4.0 }
                else { 1.0 }
            }
            CpuArch::Arm64 => {
                // Apple Silicon NEON is highly efficient
                // M-series chips have excellent memory bandwidth
                if self.has_amx { 8.0 }  // M4 with AMX
                else { 4.0 }              // M1/M2/M3 with NEON
            }
            CpuArch::Unknown => 1.0,
        }
    }

    pub fn print_report(&self) {
        println!("CPU Capabilities:");
        println!("  Vendor:     {}", self.vendor);
        println!("  Model:      {}", self.model);
        println!("  Arch:       {}", match self.arch {
            CpuArch::X86_64 => "x86_64 (Intel)",
            CpuArch::Arm64 => "arm64 (Apple Silicon)",
            CpuArch::Unknown => "Unknown",
        });
        println!("  Cores:      {}", self.core_count);

        match self.arch {
            CpuArch::X86_64 => {
                println!("  AVX:        {}", if self.has_avx { "Yes" } else { "No" });
                println!("  AVX2:       {}", if self.has_avx2 { "Yes (8x SIMD)" } else { "No" });
                println!("  AVX-512:    {}", if self.has_avx512 { "Yes (16x SIMD)" } else { "No" });
                println!("  AVX-VNNI:   {}", if self.has_avx_vnni { "Yes (AI Accel)" } else { "No" });
                println!("  Intel NPU:  {}", if self.has_npu { "Yes" } else { "No" });
            }
            CpuArch::Arm64 => {
                if self.perf_core_count > 0 || self.efficiency_core_count > 0 {
                    println!("  P-Cores:    {}", self.perf_core_count);
                    println!("  E-Cores:    {}", self.efficiency_core_count);
                }
                println!("  NEON:       {}", if self.has_neon { "Yes (128-bit SIMD)" } else { "No" });
                println!("  AMX:        {}", if self.has_amx { "Yes (Matrix Accel)" } else { "No" });
                println!("  Neural Eng: {}", if self.has_npu { "Yes (16-core ANE)" } else { "No" });
            }
            CpuArch::Unknown => {}
        }

        println!("  Est Speedup: {:.0}x", self.estimated_speedup());
    }
}

impl Default for CpuCapabilities {
    fn default() -> Self { Self::detect() }
}
