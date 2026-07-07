//! Environment fingerprint (SPEC §3.1, §7): every reported number carries
//! the machine state it was measured under. Collection is best-effort —
//! a field we cannot read is recorded as "unknown", never omitted.

use serde::Serialize;
use std::fs;

#[derive(Debug, Clone, Serialize)]
pub struct EnvFingerprint {
    pub kernel: String,
    pub cpu_model: String,
    pub governor: String,
    pub smt: String,
    pub turbo: String,
    /// ASLR handling mode actually in effect for this session (SPEC §7:
    /// randomize-and-aggregate vs. fixed; document, don't assume).
    pub aslr: String,
    pub cpu_pinning: String,
}

fn read_trimmed(path: &str) -> String {
    fs::read_to_string(path)
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}

impl EnvFingerprint {
    pub fn collect(pinning: &str, aslr_mode: &str) -> Self {
        let cpu_model = fs::read_to_string("/proc/cpuinfo")
            .ok()
            .and_then(|s| {
                s.lines()
                    .find(|l| l.starts_with("model name"))
                    .and_then(|l| l.split(':').nth(1))
                    .map(|v| v.trim().to_string())
            })
            .unwrap_or_else(|| "unknown".to_string());
        Self {
            kernel: read_trimmed("/proc/sys/kernel/osrelease"),
            cpu_model,
            governor: read_trimmed("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor"),
            smt: read_trimmed("/sys/devices/system/cpu/smt/active"),
            turbo: read_trimmed("/sys/devices/system/cpu/intel_pstate/no_turbo"),
            aslr: format!(
                "mode={aslr_mode} randomize_va_space={}",
                read_trimmed("/proc/sys/kernel/randomize_va_space")
            ),
            cpu_pinning: pinning.to_string(),
        }
    }
}
