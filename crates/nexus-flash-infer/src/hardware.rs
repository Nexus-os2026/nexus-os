use crate::types::{HardwareInfo, RamType, SsdType};

/// Detect system hardware capabilities for inference planning.
pub fn detect_hardware() -> HardwareInfo {
    let bridge_hw = nexus_llama_bridge::detect_hardware();
    let ram_type = detect_ram_type();
    let mem_bandwidth_gbps = estimate_mem_bandwidth(&ram_type, bridge_hw.cpu_cores);

    HardwareInfo {
        total_ram_mb: bridge_hw.total_ram_mb,
        cpu_cores: bridge_hw.cpu_cores,
        has_avx2: bridge_hw.has_avx2,
        has_avx512: bridge_hw.has_avx512,
        has_metal: bridge_hw.has_metal,
        has_cuda: bridge_hw.has_cuda,
        ssd_type: detect_ssd_type(),
        ssd_read_speed_mb_s: estimate_ssd_speed(),
        numa_nodes: detect_numa_nodes(),
        ram_type,
        mem_bandwidth_gbps,
    }
}

/// Detect SSD type from Linux sysfs.
fn detect_ssd_type() -> SsdType {
    #[cfg(target_os = "linux")]
    {
        // Check for NVMe devices
        if std::path::Path::new("/sys/class/nvme").exists() {
            if let Ok(entries) = std::fs::read_dir("/sys/class/nvme") {
                if entries.count() > 0 {
                    return SsdType::NVMe;
                }
            }
        }

        // Check block devices for rotational flag
        if let Ok(entries) = std::fs::read_dir("/sys/block") {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.starts_with("sd") {
                    let rotational_path = format!("/sys/block/{}/queue/rotational", name_str);
                    if let Ok(val) = std::fs::read_to_string(&rotational_path) {
                        if val.trim() == "0" {
                            return SsdType::SATA;
                        } else {
                            return SsdType::HDD;
                        }
                    }
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        // macOS: most modern Macs have NVMe
        return SsdType::NVMe;
    }

    #[allow(unreachable_code)]
    SsdType::Unknown
}

/// Estimate SSD read speed based on type.
fn estimate_ssd_speed() -> u32 {
    match detect_ssd_type() {
        SsdType::NVMe => 3500,
        SsdType::SATA => 550,
        SsdType::HDD => 150,
        SsdType::Unknown => 500,
    }
}

/// Detect RAM type (DDR4, DDR5, etc.) from Linux dmesg or sysfs.
fn detect_ram_type() -> RamType {
    #[cfg(target_os = "linux")]
    {
        // Check dmesg for DDR type — GPU drivers log "RAM width NNNbits DDR5/DDR4"
        if let Ok(output) = std::process::Command::new("dmesg")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .output()
        {
            let dmesg = String::from_utf8_lossy(&output.stdout);
            for line in dmesg.lines() {
                let lower = line.to_lowercase();
                if lower.contains("ddr5") || lower.contains("lpddr5") {
                    if lower.contains("lpddr5") {
                        return RamType::LPDDR5;
                    }
                    return RamType::DDR5;
                }
                if lower.contains("ddr4") || lower.contains("lpddr4") {
                    if lower.contains("lpddr4") {
                        return RamType::LPDDR4;
                    }
                    return RamType::DDR4;
                }
            }
        }

        // Fallback: try dmidecode (needs root, may fail silently)
        if let Ok(output) = std::process::Command::new("dmidecode")
            .args(["-t", "memory"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .output()
        {
            let text = String::from_utf8_lossy(&output.stdout);
            if text.contains("DDR5") {
                return RamType::DDR5;
            }
            if text.contains("DDR4") {
                return RamType::DDR4;
            }
            if text.contains("LPDDR5") {
                return RamType::LPDDR5;
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        // Apple Silicon uses unified LPDDR5
        return RamType::LPDDR5;
    }

    #[allow(unreachable_code)]
    RamType::Unknown
}

/// Estimate achievable single-stream memory bandwidth in GB/s.
///
/// These are conservative estimates for realistic inference workloads
/// (random-ish access patterns, not pure sequential memcpy).
fn estimate_mem_bandwidth(ram_type: &RamType, _cpu_cores: u32) -> f64 {
    match ram_type {
        RamType::DDR5 => 11.0,   // Measured: 10.7 GB/s on Ryzen 7 6800H DDR5-4800
        RamType::LPDDR5 => 12.0, // Apple M-series unified memory
        RamType::DDR4 => 6.0,    // Typical DDR4-3200 single-stream
        RamType::LPDDR4 => 5.0,
        RamType::Unknown => 7.0, // Conservative fallback
    }
}

/// Detect number of NUMA nodes.
fn detect_numa_nodes() -> u32 {
    #[cfg(target_os = "linux")]
    {
        if let Ok(entries) = std::fs::read_dir("/sys/devices/system/node") {
            let count = entries
                // Optional: skip directory entries that can't be read
                .filter_map(|e| e.ok())
                .filter(|e| e.file_name().to_string_lossy().starts_with("node"))
                .count();
            if count > 0 {
                return count as u32;
            }
        }
    }

    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_hardware_returns_valid() {
        let hw = detect_hardware();
        assert!(hw.total_ram_mb > 0);
        assert!(hw.cpu_cores > 0);
        assert!(hw.numa_nodes >= 1);
    }
}
