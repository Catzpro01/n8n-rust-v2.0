// SPDX-License-Identifier: AGPL-3.0-or-later

use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct ResourceIdentity {
    pub cgroup_version: u8,
    pub cgroup_path: String,
    pub cpu: ControllerIdentity,
    pub memory: ControllerIdentity,
    pub tasks: ControllerIdentity,
}

#[derive(Debug, Clone, Serialize)]
pub struct ControllerIdentity {
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub values: BTreeMap<String, serde_json::Value>,
}

pub fn discover(configured_directory: Option<&Path>) -> ResourceIdentity {
    let (directory, display_path) = match configured_directory {
        Some(directory) => (directory.to_path_buf(), directory.display().to_string()),
        None => self_cgroup_directory()
            .unwrap_or_else(|| (PathBuf::from("/sys/fs/cgroup"), "unresolved".into())),
    };

    ResourceIdentity {
        cgroup_version: 2,
        cgroup_path: display_path,
        cpu: cpu_identity(&directory),
        memory: memory_identity(&directory),
        tasks: tasks_identity(&directory),
    }
}

fn self_cgroup_directory() -> Option<(PathBuf, String)> {
    let membership = fs::read_to_string("/proc/self/cgroup").ok()?;
    let relative = membership
        .lines()
        .find_map(|line| line.strip_prefix("0::"))?;
    let relative = relative.trim_start_matches('/');
    let directory = Path::new("/sys/fs/cgroup").join(relative);
    Some((directory, format!("/{relative}")))
}

fn cpu_identity(directory: &Path) -> ControllerIdentity {
    let Some(cpu_max) = read(directory, "cpu.max") else {
        return unavailable("cpu.max unavailable");
    };
    let mut values = BTreeMap::new();
    let fields: Vec<_> = cpu_max.split_whitespace().collect();
    if fields.len() == 2 {
        if let Ok(period) = fields[1].parse::<u64>() {
            values.insert("period_usec".into(), period.into());
            if fields[0] == "max" {
                values.insert("quota_cores".into(), serde_json::Value::Null);
            } else if let Ok(quota) = fields[0].parse::<u64>() {
                values.insert("quota_usec".into(), quota.into());
                values.insert("quota_cores".into(), (quota as f64 / period as f64).into());
            }
        }
    }
    if let Some(stat) = read(directory, "cpu.stat") {
        for line in stat.lines() {
            let mut fields = line.split_whitespace();
            if let (Some(name), Some(value)) = (fields.next(), fields.next()) {
                if let Ok(value) = value.parse::<u64>() {
                    values.insert(name.into(), value.into());
                }
            }
        }
    }
    ControllerIdentity {
        available: true,
        reason: None,
        values,
    }
}

fn memory_identity(directory: &Path) -> ControllerIdentity {
    let Some(current) = read_number_or_max(directory, "memory.current") else {
        return unavailable("memory.current unavailable");
    };
    let mut values = BTreeMap::new();
    values.insert("current_bytes".into(), current);
    for (file, key) in [
        ("memory.max", "max_bytes"),
        ("memory.high", "high_bytes"),
        ("memory.peak", "peak_bytes"),
        ("memory.swap.max", "swap_max_bytes"),
    ] {
        if let Some(value) = read_number_or_max(directory, file) {
            values.insert(key.into(), value);
        }
    }
    ControllerIdentity {
        available: true,
        reason: None,
        values,
    }
}

fn tasks_identity(directory: &Path) -> ControllerIdentity {
    let Some(maximum) = read_number_or_max(directory, "pids.max") else {
        return unavailable("pids.max unavailable");
    };
    let mut values = BTreeMap::new();
    values.insert("max".into(), maximum);
    if let Some(current) = read_number_or_max(directory, "pids.current") {
        values.insert("current".into(), current);
    }
    ControllerIdentity {
        available: true,
        reason: None,
        values,
    }
}

fn unavailable(reason: &str) -> ControllerIdentity {
    ControllerIdentity {
        available: false,
        reason: Some(reason.into()),
        values: BTreeMap::new(),
    }
}

fn read(directory: &Path, name: &str) -> Option<String> {
    fs::read_to_string(directory.join(name))
        .ok()
        .map(|value| value.trim().to_owned())
}

fn read_number_or_max(directory: &Path, name: &str) -> Option<serde_json::Value> {
    let value = read(directory, name)?;
    if value == "max" {
        Some(serde_json::Value::Null)
    } else {
        value.parse::<u64>().ok().map(Into::into)
    }
}
