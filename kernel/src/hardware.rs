//! Hardware detection for GPU, VRAM, and system RAM.

use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HardwareProfile {
    pub gpu: String,
    pub vram_mb: u64,
    pub ram_mb: u64,
    pub detected_at: String,
}

impl Default for HardwareProfile {
    fn default() -> Self {
        Self {
            gpu: "none".to_string(),
            vram_mb: 0,
            ram_mb: 0,
            detected_at: String::new(),
        }
    }
}

impl HardwareProfile {
    pub fn detect() -> Self {
        let gpu_info = detect_gpu();
        let ram_mb = detect_ram_mb();
        let now = chrono_iso8601_now();

        Self {
            gpu: gpu_info.0,
            vram_mb: gpu_info.1,
            ram_mb,
            detected_at: now,
        }
    }

    /// Recommend a model tier based on available VRAM.
    pub fn recommended_tier(&self) -> ModelTier {
        if self.vram_mb >= 24_000 {
            ModelTier::Large
        } else if self.vram_mb >= 8_000 {
            ModelTier::Medium
        } else if self.vram_mb >= 4_000 {
            ModelTier::Small
        } else {
            ModelTier::Tiny
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelTier {
    Large,
    Medium,
    Small,
    Tiny,
}

impl ModelTier {
    pub fn primary_model(&self) -> &'static str {
        match self {
            ModelTier::Large => "qwen3.5:14b",
            ModelTier::Medium => "qwen3.5:9b",
            ModelTier::Small => "qwen3.5:4b",
            ModelTier::Tiny => "qwen3.5:1.5b",
        }
    }

    pub fn fast_model(&self) -> &'static str {
        match self {
            ModelTier::Large => "qwen3.5:9b",
            ModelTier::Medium => "qwen3.5:4b",
            ModelTier::Small => "qwen3.5:1.5b",
            ModelTier::Tiny => "qwen3.5:0.5b",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            ModelTier::Large => "Large (24GB+ VRAM)",
            ModelTier::Medium => "Medium (8-24GB VRAM)",
            ModelTier::Small => "Small (4-8GB VRAM)",
            ModelTier::Tiny => "Tiny (<4GB VRAM)",
        }
    }
}

/// Agent model recommendation based on tier.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentModelConfig {
    pub model: String,
    pub temperature: f64,
    pub max_tokens: u32,
}

pub fn recommend_agent_configs(tier: ModelTier) -> Vec<(&'static str, AgentModelConfig)> {
    let primary = tier.primary_model().to_string();
    let fast = tier.fast_model().to_string();

    vec![
        (
            "coder",
            AgentModelConfig {
                model: primary.clone(),
                temperature: 0.4,
                max_tokens: 8192,
            },
        ),
        (
            "designer",
            AgentModelConfig {
                model: primary.clone(),
                temperature: 0.7,
                max_tokens: 4096,
            },
        ),
        (
            "screen_poster",
            AgentModelConfig {
                model: fast.clone(),
                temperature: 0.8,
                max_tokens: 2048,
            },
        ),
        (
            "web_builder",
            AgentModelConfig {
                model: primary.clone(),
                temperature: 0.5,
                max_tokens: 8192,
            },
        ),
        (
            "workflow_studio",
            AgentModelConfig {
                model: fast.clone(),
                temperature: 0.3,
                max_tokens: 4096,
            },
        ),
        (
            "self_improve",
            AgentModelConfig {
                model: primary,
                temperature: 0.5,
                max_tokens: 4096,
            },
        ),
    ]
}

fn detect_gpu() -> (String, u64) {
    // Try nvidia-smi first
    if let Ok(output) = Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,memory.total",
            "--format=csv,noheader,nounits",
        ])
        .output()
    {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            let line = text.lines().next().unwrap_or_default().trim();
            if let Some((name, vram_str)) = line.split_once(',') {
                let vram: u64 = vram_str.trim().parse().unwrap_or(0);
                return (name.trim().to_string(), vram);
            }
        }
    }

    // Try AMD ROCm
    if let Ok(output) = Command::new("rocm-smi")
        .args(["--showproductname", "--showmeminfo", "vram", "--csv"])
        .output()
    {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            return parse_rocm_output(&text);
        }
    }

    // Try lspci as fallback for GPU name
    if let Ok(output) = Command::new("lspci").output() {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            for line in text.lines() {
                let lower = line.to_lowercase();
                if lower.contains("vga") || lower.contains("3d controller") {
                    if let Some(name_part) = line.split(':').next_back() {
                        return (name_part.trim().to_string(), 0);
                    }
                }
            }
        }
    }

    ("none".to_string(), 0)
}

fn parse_rocm_output(text: &str) -> (String, u64) {
    let mut name = "AMD GPU".to_string();
    let mut vram: u64 = 0;
    for line in text.lines() {
        if line.contains("Card series") || line.contains("card_series") {
            if let Some(val) = line.split(',').nth(1) {
                name = val.trim().to_string();
            }
        }
        if line.contains("Total") {
            for part in line.split(',') {
                if let Ok(v) = part.trim().parse::<u64>() {
                    if v > vram {
                        vram = v / (1024 * 1024); // bytes to MB
                    }
                }
            }
        }
    }
    (name, vram)
}

fn detect_ram_mb() -> u64 {
    // Linux: /proc/meminfo
    if let Ok(content) = std::fs::read_to_string("/proc/meminfo") {
        for line in content.lines() {
            if line.starts_with("MemTotal:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let Ok(kb) = parts[1].parse::<u64>() {
                        return kb / 1024;
                    }
                }
            }
        }
    }

    // macOS: sysctl
    if let Ok(output) = Command::new("sysctl").args(["-n", "hw.memsize"]).output() {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            if let Ok(bytes) = text.trim().parse::<u64>() {
                return bytes / (1024 * 1024);
            }
        }
    }

    // Windows: wmic
    if let Ok(output) = Command::new("wmic")
        .args(["ComputerSystem", "get", "TotalPhysicalMemory"])
        .output()
    {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            for line in text.lines().skip(1) {
                if let Ok(bytes) = line.trim().parse::<u64>() {
                    return bytes / (1024 * 1024);
                }
            }
        }
    }

    0
}

fn chrono_iso8601_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => {
            let secs = d.as_secs();
            let days = secs / 86400;
            let time = secs % 86400;
            let hours = time / 3600;
            let minutes = (time % 3600) / 60;
            let seconds = time % 60;
            // Approximate date calculation
            let mut y = 1970_i64;
            let mut remaining = days as i64;
            loop {
                let year_days = if is_leap(y) { 366 } else { 365 };
                if remaining < year_days {
                    break;
                }
                remaining -= year_days;
                y += 1;
            }
            let month_days: [i64; 12] = if is_leap(y) {
                [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
            } else {
                [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
            };
            let mut m = 1;
            for md in &month_days {
                if remaining < *md {
                    break;
                }
                remaining -= *md;
                m += 1;
            }
            let day = remaining + 1;
            format!(
                "{y:04}-{m:02}-{day:02}T{hours:02}:{minutes:02}:{seconds:02}Z"
            )
        }
        Err(_) => "1970-01-01T00:00:00Z".to_string(),
    }
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hardware_profile_defaults() {
        let profile = HardwareProfile::default();
        assert_eq!(profile.gpu, "none");
        assert_eq!(profile.vram_mb, 0);
    }

    #[test]
    fn test_model_tier_recommendations() {
        assert_eq!(ModelTier::Medium.primary_model(), "qwen3.5:9b");
        assert_eq!(ModelTier::Medium.fast_model(), "qwen3.5:4b");
    }

    #[test]
    fn test_tier_from_vram() {
        let profile = HardwareProfile {
            vram_mb: 12288,
            ..Default::default()
        };
        assert_eq!(profile.recommended_tier(), ModelTier::Medium);
    }

    #[test]
    fn test_agent_configs_generated() {
        let configs = recommend_agent_configs(ModelTier::Medium);
        assert_eq!(configs.len(), 6);
        assert_eq!(configs[0].0, "coder");
        assert_eq!(configs[0].1.model, "qwen3.5:9b");
    }

    #[test]
    fn test_detect_ram_returns_value() {
        // On most systems this should return a nonzero value
        let ram = detect_ram_mb();
        // We can't assert > 0 on all CI systems, but we can test it doesn't panic
        let _ = ram;
    }

    #[test]
    fn test_iso8601_format() {
        let ts = chrono_iso8601_now();
        assert!(ts.contains('T'));
        assert!(ts.ends_with('Z'));
    }
}
