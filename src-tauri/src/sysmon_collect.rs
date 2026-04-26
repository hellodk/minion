//! System metric collection via sysinfo + optional nvml.

use serde::{Deserialize, Serialize};
use sysinfo::{CpuRefreshKind, Disks, MemoryRefreshKind, Networks, ProcessStatus, RefreshKind, System};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskInfo {
    pub mount: String,
    pub used_gb: f64,
    pub total_gb: f64,
    pub read_bps: u64,
    pub write_bps: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    pub name: String,
    pub util_pct: f32,
    pub vram_used_mb: u64,
    pub vram_total_mb: u64,
    pub temp_c: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetInfo {
    pub iface: String,
    pub rx_bps: u64,
    pub tx_bps: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cpu_pct: f32,
    pub ram_mb: u64,
    pub status: String,
    pub user_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemSnapshot {
    pub cpu_pct: f32,
    pub ram_used_mb: u64,
    pub ram_total_mb: u64,
    pub swap_used_mb: u64,
    pub load_avg_1: Option<f64>,
    pub disks: Vec<DiskInfo>,
    pub gpus: Vec<GpuInfo>,
    pub net: Vec<NetInfo>,
}

/// Holds sysinfo state across calls so CPU % is computed over a real interval.
pub struct Collector {
    sys: System,
    disks: Disks,
    networks: Networks,
}

impl Collector {
    pub fn new() -> Self {
        let mut sys = System::new_with_specifics(
            RefreshKind::new()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything()),
        );
        sys.refresh_cpu_usage();
        std::thread::sleep(std::time::Duration::from_millis(200));
        sys.refresh_cpu_usage();

        Self {
            sys,
            disks: Disks::new_with_refreshed_list(),
            networks: Networks::new_with_refreshed_list(),
        }
    }

    /// Collect a fresh snapshot. Call every 5 seconds for accurate CPU %.
    pub fn snapshot(&mut self) -> SystemSnapshot {
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();
        self.disks.refresh();
        self.networks.refresh();

        let cpu_pct = self.sys.global_cpu_info().cpu_usage();
        let ram_used_mb = self.sys.used_memory() / 1024 / 1024;
        let ram_total_mb = self.sys.total_memory() / 1024 / 1024;
        let swap_used_mb = self.sys.used_swap() / 1024 / 1024;

        let load_avg_1 = {
            let la = System::load_average();
            if la.one > 0.0 { Some(la.one) } else { None }
        };

        let disks = self.disks.iter().map(|d| DiskInfo {
            mount: d.mount_point().to_string_lossy().to_string(),
            used_gb: (d.total_space().saturating_sub(d.available_space())) as f64 / 1e9,
            total_gb: d.total_space() as f64 / 1e9,
            read_bps: 0,
            write_bps: 0,
        }).collect();

        let net = self.networks.iter().map(|(iface, data)| NetInfo {
            iface: iface.clone(),
            rx_bps: data.received(),
            tx_bps: data.transmitted(),
        }).collect();

        let gpus = collect_gpus();

        SystemSnapshot {
            cpu_pct,
            ram_used_mb,
            ram_total_mb,
            swap_used_mb,
            load_avg_1,
            disks,
            gpus,
            net,
        }
    }

    /// Collect top-20 processes sorted by CPU descending.
    pub fn top_processes(&mut self) -> Vec<ProcessInfo> {
        self.sys.refresh_processes();
        let mut procs: Vec<ProcessInfo> = self.sys.processes().values().map(|p| {
            let status = match p.status() {
                ProcessStatus::Zombie => "zombie",
                ProcessStatus::Sleep => "sleeping",
                ProcessStatus::Stop => "stopped",
                _ => "running",
            }.to_string();
            ProcessInfo {
                pid: p.pid().as_u32(),
                name: p.name().to_string(),
                cpu_pct: p.cpu_usage(),
                ram_mb: p.memory() / 1024 / 1024,
                status,
                user_name: p.user_id().map(|u| u.to_string()),
            }
        }).collect();
        procs.sort_by(|a, b| b.cpu_pct.partial_cmp(&a.cpu_pct).unwrap_or(std::cmp::Ordering::Equal));
        procs.truncate(20);
        procs
    }
}

fn collect_gpus() -> Vec<GpuInfo> {
    #[cfg(feature = "nvidia")]
    {
        if let Ok(nvml) = nvml_wrapper::Nvml::init() {
            if let Ok(count) = nvml.device_count() {
                let mut gpus = Vec::new();
                for i in 0..count {
                    if let Ok(dev) = nvml.device_by_index(i) {
                        let name = dev.name().unwrap_or_default();
                        let util = dev.utilization_rates().map(|u| u.gpu as f32).unwrap_or(0.0);
                        let mem = dev.memory_info().ok();
                        let temp = dev.temperature(nvml_wrapper::enum_wrappers::device::TemperatureSensor::Gpu)
                            .map(|t| t as f32).ok();
                        gpus.push(GpuInfo {
                            name,
                            util_pct: util,
                            vram_used_mb: mem.as_ref().map(|m| m.used / 1024 / 1024).unwrap_or(0),
                            vram_total_mb: mem.as_ref().map(|m| m.total / 1024 / 1024).unwrap_or(0),
                            temp_c: temp,
                        });
                    }
                }
                return gpus;
            }
        }
    }
    Vec::new()
}
