use std::collections::HashMap;
use tauri::Emitter;
use thiserror::Error;
use tokio::task::JoinHandle;

use crate::ssh::client::{SharedHandle, exec_on_connection};
use crate::state::{AppState, SystemStats};
use tauri::Manager;

#[derive(Debug, Error)]
pub enum CollectorError {
    #[error("Not connected to {0}")]
    NotConnected(String),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("Command failed: {0}")]
    CommandFailed(String),
}

pub struct MonitoringCollector {
    tasks: HashMap<String, JoinHandle<()>>,
}

impl MonitoringCollector {
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
        }
    }

    pub fn start(
        &mut self,
        connection_id: &str,
        handle: SharedHandle,
        app_handle: tauri::AppHandle,
    ) {
        // Stop existing task if any
        self.stop(connection_id);

        let id = connection_id.to_string();
        let task = tokio::spawn(async move {
            monitoring_loop(id, handle, app_handle).await;
        });
        self.tasks.insert(connection_id.to_string(), task);
        tracing::info!("Started monitoring for {}", connection_id);
    }

    pub fn stop(&mut self, connection_id: &str) {
        if let Some(task) = self.tasks.remove(connection_id) {
            task.abort();
            tracing::info!("Stopped monitoring for {}", connection_id);
        }
    }

    pub fn is_monitoring(&self, connection_id: &str) -> bool {
        self.tasks.contains_key(connection_id)
    }
}

impl Default for MonitoringCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Single command that grabs memory, disk, and users in one shot.
/// CPU is handled separately with two snapshots.
/// Uses `w -hs` (more reliable than `who` on minimal systems), falls back to `who`.
const STATS_COMMAND: &str = concat!(
    "cat /proc/meminfo 2>/dev/null; echo '===REACH_SEP==='; ",
    "df -P / 2>/dev/null; echo '===REACH_SEP==='; ",
    "w -hs 2>/dev/null || who 2>/dev/null"
);

/// Read the first "cpu " line from /proc/stat.
const CPU_COMMAND: &str = "head -1 /proc/stat";

#[derive(Default, Clone, Copy)]
struct CpuSnapshot {
    total: u64,
    idle: u64,
}

async fn monitoring_loop(
    connection_id: String,
    handle: SharedHandle,
    app_handle: tauri::AppHandle,
) {
    let event_name = format!("monitoring-{}", connection_id);
    let mut prev_cpu = CpuSnapshot::default();

    tracing::info!("Monitoring loop starting for {}", connection_id);

    loop {
        // Take first CPU snapshot
        let cpu1 = read_cpu_snapshot(&handle).await.unwrap_or_default();
        tracing::debug!("CPU snapshot 1: total={}, idle={}", cpu1.total, cpu1.idle);

        // Small delay between CPU readings for delta
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // Take second CPU snapshot + all other stats
        let cpu2 = read_cpu_snapshot(&handle).await.unwrap_or_default();

        // Compute CPU% from delta (or from previous loop iteration if delta is 0)
        let cpu = if cpu2.total > cpu1.total {
            let total_delta = cpu2.total - cpu1.total;
            let idle_delta = cpu2.idle - cpu1.idle;
            ((total_delta - idle_delta) as f64 / total_delta as f64 * 100.0).round()
        } else if cpu2.total > prev_cpu.total {
            let total_delta = cpu2.total - prev_cpu.total;
            let idle_delta = cpu2.idle - prev_cpu.idle;
            ((total_delta - idle_delta) as f64 / total_delta as f64 * 100.0).round()
        } else {
            0.0
        };
        prev_cpu = cpu2;

        // Fetch memory, disk, users
        match exec_on_connection(&handle, STATS_COMMAND).await {
            Ok(output) => {
                tracing::info!("Monitoring data: {} bytes, sections: {}", output.len(), output.matches("===REACH_SEP===").count());

                let sections: Vec<&str> = output.split("===REACH_SEP===").collect();

                let (ram_total, ram_used) = sections
                    .first()
                    .map(|s| parse_memory(s))
                    .unwrap_or((0, 0));
                let ram = if ram_total > 0 {
                    (ram_used as f64 / ram_total as f64) * 100.0
                } else {
                    0.0
                };
                let disk = sections.get(1).map(|s| parse_disk(s)).unwrap_or(0.0);
                let users = sections
                    .get(2)
                    .map(|s| parse_users(s))
                    .unwrap_or_default();

                let stats = SystemStats {
                    cpu,
                    ram,
                    ram_total,
                    ram_used,
                    disk,
                    users,
                };

                // Store in AppState for polling
                {
                    let state = app_handle.state::<AppState>();
                    let mut monitoring = state.monitoring.write().await;
                    monitoring.insert(connection_id.clone(), stats.clone());
                }

                // Also emit event for real-time listeners
                let _ = app_handle.emit(&event_name, &stats);
            }
            Err(e) => {
                tracing::warn!(
                    "Monitoring command failed for {}: {}",
                    connection_id,
                    e
                );
            }
        }

        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    }
}

async fn read_cpu_snapshot(handle: &SharedHandle) -> Option<CpuSnapshot> {
    let output = exec_on_connection(handle, CPU_COMMAND).await.ok()?;
    for line in output.lines() {
        if line.starts_with("cpu ") {
            let values: Vec<u64> = line
                .split_whitespace()
                .skip(1)
                .filter_map(|s| s.parse().ok())
                .collect();

            if values.len() >= 5 {
                let total: u64 = values.iter().sum();
                // idle (index 3) + iowait (index 4)
                let idle = values[3] + values.get(4).copied().unwrap_or(0);
                return Some(CpuSnapshot { total, idle });
            }
        }
    }
    None
}

fn parse_memory(output: &str) -> (u64, u64) {
    let mut total: u64 = 0;
    let mut free: u64 = 0;
    let mut available: u64 = 0;
    let mut buffers: u64 = 0;
    let mut cached: u64 = 0;
    let mut has_available = false;

    for line in output.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            let value: u64 = parts[1].parse().unwrap_or(0);
            match parts[0] {
                "MemTotal:" => total = value * 1024,
                "MemFree:" => free = value * 1024,
                "MemAvailable:" => {
                    available = value * 1024;
                    has_available = true;
                }
                "Buffers:" => buffers = value * 1024,
                "Cached:" => cached = value * 1024,
                _ => {}
            }
        }
    }

    // Prefer MemAvailable (modern kernels), fall back to free+buffers+cached
    let used = if has_available {
        total.saturating_sub(available)
    } else {
        total.saturating_sub(free + buffers + cached)
    };
    (total, used)
}

fn parse_disk(output: &str) -> f64 {
    for line in output.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 5 {
            if let Some(pct) = parts[4].strip_suffix('%') {
                return pct.parse().unwrap_or(0.0);
            }
        }
    }
    0.0
}

fn parse_users(output: &str) -> Vec<String> {
    // `w -hs` format: "root  pts/0  192.168.1.1  0.00s -bash"
    // `who` format:   "root  pts/0  2024-01-01 12:00 (192.168.1.1)"
    let mut users: Vec<String> = output
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                return None;
            }
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() < 2 {
                return parts.first().map(|s| s.to_string());
            }

            let user = parts[0];
            let tty = parts[1];

            // Extract source IP/hostname:
            // `who`: last field in parentheses, e.g. (178.202.144.245)
            // `w -hs`: 3rd field is the source host/IP
            let source = parts
                .iter()
                .rev()
                .find(|p| p.starts_with('(') && p.ends_with(')'))
                .map(|p| p[1..p.len() - 1].to_string())
                .or_else(|| {
                    if parts.len() >= 3
                        && (parts[2].contains('.') || parts[2].contains(':'))
                    {
                        Some(parts[2].to_string())
                    } else {
                        None
                    }
                });

            match source {
                Some(src) => Some(format!("{}@{} ({})", user, tty, src)),
                None => Some(format!("{}@{}", user, tty)),
            }
        })
        .collect();
    users.sort();
    users.dedup();
    users
}
