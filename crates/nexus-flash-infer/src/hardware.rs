use crate::types::{HardwareInfo, SsdType};

/// Detect system hardware capabilities for inference planning.
pub fn detect_hardware() -> HardwareInfo {
    let bridge_hw = nexus_llama_bridge::detect_hardware();

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

/// Detect number of NUMA nodes.
fn detect_numa_nodes() -> u32 {
    #[cfg(target_os = "linux")]
    {
        if let Ok(entries) = std::fs::read_dir("/sys/devices/system/node") {
            let count = entries
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
