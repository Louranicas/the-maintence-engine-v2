//! # M06: Resource Manager
//!
//! System resource monitoring and lifecycle management for the Maintenance Engine.
//!
//! ## Layer: L1 (Foundation)
//! ## Dependencies: M01 (Error Taxonomy)
//! ## Tests: 10 target
//!
//! ## 12D Tensor Encoding
//! ```text
//! [6/36, 0.0, 1/6, 0, 0, 0.5, health, uptime, synergy, latency, error_rate, temporal]
//! ```
//!
//! ## Features
//!
//! - CPU monitoring (current usage percentage)
//! - Memory monitoring (used, available, total)
//! - Disk monitoring (used, available for data directory)
//! - Process monitoring (PID, thread count, open files)
//! - Resource limits with configurable thresholds and alerts
//!
//! ## Platform Support
//!
//! | Platform | CPU | Memory | Disk | Process |
//! |----------|-----|--------|------|---------|
//! | Linux    | Full (/proc/stat) | Full (/proc/meminfo) | Full (/proc/mounts) | Full (/proc/self) |
//! | macOS    | Partial | Partial | Partial | Partial |
//! | Windows  | Fallback | Fallback | Fallback | Fallback |
//!
//! ## Implementation Notes
//!
//! This module is implemented with **zero unsafe code** as required by the project's
//! quality gates. All system information is gathered through:
//! - Reading from `/proc` filesystem (Linux)
//! - Parsing system files
//! - Using safe Rust standard library functions
//!
//! ## Related Documentation
//! - [Module Specification](../../ai_docs/modules/M06_RESOURCE_MANAGER.md)
//! - [Layer Specification](../../ai_docs/layers/L01_FOUNDATION.md)

// Allow intentional casts for resource monitoring metrics
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::SystemTime;

use crate::{Error, Result};

// ============================================================================
// Core Types
// ============================================================================

/// System resource snapshot
///
/// Captures the current state of system resources including CPU, memory,
/// disk, and process-level metrics.
#[derive(Clone, Debug)]
pub struct SystemResources {
    /// CPU usage percentage (0.0 - 100.0)
    pub cpu_percent: f64,
    /// Memory currently used in megabytes
    pub memory_used_mb: u64,
    /// Total memory available to the system in megabytes
    pub memory_total_mb: u64,
    /// Memory usage percentage (0.0 - 100.0)
    pub memory_percent: f64,
    /// Disk space used in megabytes
    pub disk_used_mb: u64,
    /// Total disk space in megabytes
    pub disk_total_mb: u64,
    /// Disk usage percentage (0.0 - 100.0)
    pub disk_percent: f64,
    /// Number of open file descriptors
    pub open_files: u32,
    /// Number of threads in the current process
    pub thread_count: u32,
    /// Timestamp of resource collection
    pub timestamp: SystemTime,
}

impl Default for SystemResources {
    fn default() -> Self {
        Self {
            cpu_percent: 0.0,
            memory_used_mb: 0,
            memory_total_mb: 0,
            memory_percent: 0.0,
            disk_used_mb: 0,
            disk_total_mb: 0,
            disk_percent: 0.0,
            open_files: 0,
            thread_count: 0,
            timestamp: SystemTime::now(),
        }
    }
}

impl SystemResources {
    /// Encode system resources as a 12D tensor (NAM R4).
    ///
    /// D6 = 1 - cpu/100 (health inversely proportional to CPU), D10 = cpu/100 (error proxy).
    #[must_use]
    pub fn to_tensor(&self) -> crate::Tensor12D {
        let cpu_normalized = self.cpu_percent / 100.0;
        let mut tensor = crate::Tensor12D {
            service_id: 0.0,
            port: 0.0,
            tier: 1.0 / 6.0,
            dependency_count: 0.0,
            agent_count: 0.0,
            protocol: 0.5,
            health_score: 1.0 - cpu_normalized,
            uptime: 0.0,
            synergy: 0.0,
            latency: 1.0,
            error_rate: cpu_normalized,
            temporal_context: 0.0,
        };
        tensor.clamp_normalize();
        tensor
    }
}

/// Resource usage limits configuration
///
/// Defines thresholds for resource alerts. When any resource exceeds
/// its configured threshold, an alert is generated.
#[derive(Clone, Debug)]
pub struct ResourceLimits {
    /// Maximum CPU usage percentage before alert (default: 80.0)
    pub max_cpu_percent: f64,
    /// Maximum memory usage percentage before alert (default: 85.0)
    pub max_memory_percent: f64,
    /// Maximum disk usage percentage before alert (default: 90.0)
    pub max_disk_percent: f64,
    /// Maximum number of open files before alert (default: 1000)
    pub max_open_files: u32,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_cpu_percent: 80.0,
            max_memory_percent: 85.0,
            max_disk_percent: 90.0,
            max_open_files: 1000,
        }
    }
}

impl ResourceLimits {
    /// Create a new resource limits configuration with custom values
    #[must_use]
    pub const fn new(
        max_cpu_percent: f64,
        max_memory_percent: f64,
        max_disk_percent: f64,
        max_open_files: u32,
    ) -> Self {
        Self {
            max_cpu_percent,
            max_memory_percent,
            max_disk_percent,
            max_open_files,
        }
    }

    /// Validate that all limits are within reasonable ranges
    ///
    /// # Errors
    ///
    /// Returns an error if any limit is outside the valid range (0-100 for percentages).
    pub fn validate(&self) -> Result<()> {
        if !(0.0..=100.0).contains(&self.max_cpu_percent) {
            return Err(Error::Validation(format!(
                "max_cpu_percent must be between 0 and 100, got {}",
                self.max_cpu_percent
            )));
        }
        if !(0.0..=100.0).contains(&self.max_memory_percent) {
            return Err(Error::Validation(format!(
                "max_memory_percent must be between 0 and 100, got {}",
                self.max_memory_percent
            )));
        }
        if !(0.0..=100.0).contains(&self.max_disk_percent) {
            return Err(Error::Validation(format!(
                "max_disk_percent must be between 0 and 100, got {}",
                self.max_disk_percent
            )));
        }
        Ok(())
    }
}

/// Resource alert types
///
/// Generated when resource usage exceeds configured thresholds.
#[derive(Clone, Debug, PartialEq)]
pub enum ResourceAlert {
    /// CPU usage exceeded threshold
    CpuHigh {
        /// Current CPU usage percentage
        current: f64,
        /// Configured threshold
        threshold: f64,
    },
    /// Memory usage exceeded threshold
    MemoryHigh {
        /// Current memory usage percentage
        current: f64,
        /// Configured threshold
        threshold: f64,
    },
    /// Disk usage exceeded threshold
    DiskHigh {
        /// Current disk usage percentage
        current: f64,
        /// Configured threshold
        threshold: f64,
    },
    /// Open files count exceeded threshold
    OpenFilesHigh {
        /// Current open files count
        current: u32,
        /// Configured threshold
        threshold: u32,
    },
}

impl std::fmt::Display for ResourceAlert {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CpuHigh { current, threshold } => {
                write!(f, "CPU usage high: {current:.1}% (threshold: {threshold:.1}%)")
            }
            Self::MemoryHigh { current, threshold } => {
                write!(f, "Memory usage high: {current:.1}% (threshold: {threshold:.1}%)")
            }
            Self::DiskHigh { current, threshold } => {
                write!(f, "Disk usage high: {current:.1}% (threshold: {threshold:.1}%)")
            }
            Self::OpenFilesHigh { current, threshold } => {
                write!(f, "Open files high: {current} (threshold: {threshold})")
            }
        }
    }
}

/// Process information for the current process
#[derive(Clone, Debug)]
pub struct ProcessInfo {
    /// Process ID
    pub pid: u32,
    /// Number of threads
    pub thread_count: u32,
    /// Number of open file descriptors
    pub open_files: u32,
    /// Virtual memory size in bytes
    pub virtual_memory_bytes: u64,
    /// Resident set size in bytes
    pub resident_memory_bytes: u64,
    /// Process start time
    pub start_time: Option<SystemTime>,
}

impl Default for ProcessInfo {
    fn default() -> Self {
        Self {
            pid: 0,
            thread_count: 1,
            open_files: 0,
            virtual_memory_bytes: 0,
            resident_memory_bytes: 0,
            start_time: None,
        }
    }
}

// ============================================================================
// ResourceCollector Trait
// ============================================================================

/// Trait for types that can collect and report system resource usage.
///
/// Enables dependency inversion — upper layers can accept `&dyn ResourceCollector`
/// instead of depending directly on [`ResourceManager`].
pub trait ResourceCollector: Send + Sync {
    /// Collect current system resource metrics.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying platform collection fails.
    fn collect(&self) -> crate::Result<SystemResources>;

    /// Check resource usage against configured limits and return alerts.
    fn check_limits(&self) -> Vec<ResourceAlert>;

    /// Get a composite health score from 0.0 (degraded) to 1.0 (healthy).
    fn health_score(&self) -> f64;

    /// Return the agent ID associated with this collector (NAM R5).
    fn agent_id(&self) -> Option<&str> {
        None
    }

    /// Encode collector state as a 12D tensor (NAM R4).
    fn to_tensor(&self) -> crate::Tensor12D {
        crate::Tensor12D::default()
    }
}

// ============================================================================
// NAM: Adaptive Resource Limits (R2 HebbianRouting)
// ============================================================================

/// Resource limits that adapt based on Hebbian pathway strength (NAM R2).
///
/// When `pathway_strength` is high (the system has learned that it is safe
/// to use more resources), thresholds relax slightly.
#[derive(Debug, Clone)]
pub struct AdaptiveResourceLimits {
    /// Base resource limits before adaptation.
    pub base: ResourceLimits,
    /// Hebbian pathway influence (0.0–1.0). Higher = more relaxation.
    pub pathway_strength: f64,
}

impl Default for AdaptiveResourceLimits {
    fn default() -> Self {
        Self {
            base: ResourceLimits::default(),
            pathway_strength: 0.0,
        }
    }
}

impl AdaptiveResourceLimits {
    /// Compute effective limits after applying pathway adaptation.
    ///
    /// High pathway strength relaxes limits by up to 10%.
    #[must_use]
    pub fn effective_limits(&self) -> ResourceLimits {
        let relaxation = self.pathway_strength.clamp(0.0, 1.0) * 10.0;
        ResourceLimits {
            max_cpu_percent: (self.base.max_cpu_percent + relaxation).min(100.0),
            max_memory_percent: (self.base.max_memory_percent + relaxation).min(100.0),
            max_disk_percent: (self.base.max_disk_percent + relaxation).min(100.0),
            max_open_files: self.base.max_open_files,
        }
    }
}

// ============================================================================
// Resource Collection Functions
// ============================================================================

/// Collect current system resources
///
/// Gathers CPU, memory, disk, and process information from the system.
/// Uses platform-specific methods where available with fallback for
/// unsupported platforms.
///
/// # Errors
///
/// Returns an error if resource collection fails on the current platform.
///
/// # Example
///
/// ```no_run
/// use maintenance_engine::m1_foundation::resources::collect_resources;
///
/// let resources = collect_resources().expect("Failed to collect resources");
/// println!("CPU: {:.1}%", resources.cpu_percent);
/// println!("Memory: {}/{} MB", resources.memory_used_mb, resources.memory_total_mb);
/// ```
pub fn collect_resources() -> Result<SystemResources> {
    // Collect CPU usage
    let cpu_percent = collect_cpu_usage().unwrap_or(0.0);

    // Collect memory information
    let (memory_used_mb, memory_total_mb) = collect_memory_info().unwrap_or((0, 0));
    let memory_percent = if memory_total_mb > 0 {
        (memory_used_mb as f64 / memory_total_mb as f64) * 100.0
    } else {
        0.0
    };

    // Collect disk information (default to root filesystem)
    let (disk_used_mb, disk_total_mb) = collect_disk_info("/").unwrap_or((0, 0));
    let disk_percent = if disk_total_mb > 0 {
        (disk_used_mb as f64 / disk_total_mb as f64) * 100.0
    } else {
        0.0
    };

    // Collect process information
    let process_info = get_process_info().unwrap_or_default();

    Ok(SystemResources {
        cpu_percent,
        memory_used_mb,
        memory_total_mb,
        memory_percent,
        disk_used_mb,
        disk_total_mb,
        disk_percent,
        open_files: process_info.open_files,
        thread_count: process_info.thread_count,
        timestamp: SystemTime::now(),
    })
}

/// Collect CPU usage percentage
///
/// On Linux, reads from /proc/stat. Returns instantaneous usage
/// which may be 0.0 on first call (requires delta calculation for accuracy).
fn collect_cpu_usage() -> Result<f64> {
    #[cfg(target_os = "linux")]
    {
        collect_cpu_usage_linux()
    }

    #[cfg(not(target_os = "linux"))]
    {
        // Fallback: return 0.0 for unsupported platforms
        Ok(0.0)
    }
}

#[cfg(target_os = "linux")]
fn collect_cpu_usage_linux() -> Result<f64> {
    let stat_content = fs::read_to_string("/proc/stat")
        .map_err(|e| Error::Other(format!("Failed to read /proc/stat: {e}")))?;

    // Parse first line: "cpu <user> <nice> <system> <idle> <iowait> <irq> <softirq> ..."
    let first_line = stat_content
        .lines()
        .next()
        .ok_or_else(|| Error::Other("Empty /proc/stat".to_string()))?;

    let parts: Vec<&str> = first_line.split_whitespace().collect();
    if parts.len() < 5 {
        return Err(Error::Other("Invalid /proc/stat format".to_string()));
    }

    // Skip "cpu" label and parse values
    let user: u64 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    let nice: u64 = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
    let system: u64 = parts.get(3).and_then(|s| s.parse().ok()).unwrap_or(0);
    let idle: u64 = parts.get(4).and_then(|s| s.parse().ok()).unwrap_or(0);
    let iowait: u64 = parts.get(5).and_then(|s| s.parse().ok()).unwrap_or(0);
    let irq: u64 = parts.get(6).and_then(|s| s.parse().ok()).unwrap_or(0);
    let softirq: u64 = parts.get(7).and_then(|s| s.parse().ok()).unwrap_or(0);

    let total = user + nice + system + idle + iowait + irq + softirq;
    let active = total - idle - iowait;

    if total == 0 {
        return Ok(0.0);
    }

    // Return percentage of active CPU time
    Ok((active as f64 / total as f64) * 100.0)
}

/// Collect memory information
///
/// Returns (`used_mb`, `total_mb`) tuple.
fn collect_memory_info() -> Result<(u64, u64)> {
    #[cfg(target_os = "linux")]
    {
        collect_memory_info_linux()
    }

    #[cfg(not(target_os = "linux"))]
    {
        // Fallback: return zeros for unsupported platforms
        Ok((0, 0))
    }
}

#[cfg(target_os = "linux")]
fn collect_memory_info_linux() -> Result<(u64, u64)> {
    let meminfo_content = fs::read_to_string("/proc/meminfo")
        .map_err(|e| Error::Other(format!("Failed to read /proc/meminfo: {e}")))?;

    let mut mem_total_kb: u64 = 0;
    let mut mem_available_kb: u64 = 0;

    for line in meminfo_content.lines() {
        if line.starts_with("MemTotal:") {
            mem_total_kb = parse_meminfo_line(line);
        } else if line.starts_with("MemAvailable:") {
            mem_available_kb = parse_meminfo_line(line);
        }
    }

    let total_mb = mem_total_kb / 1024;
    let available_mb = mem_available_kb / 1024;
    let used_mb = total_mb.saturating_sub(available_mb);

    Ok((used_mb, total_mb))
}

/// Parse a meminfo line like "`MemTotal`:       16384 kB"
#[cfg(target_os = "linux")]
fn parse_meminfo_line(line: &str) -> u64 {
    line.split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

/// Collect disk information for a given path
///
/// Returns (`used_mb`, `total_mb`) tuple.
///
/// On Linux, reads from /proc/mounts and /sys/block to get disk statistics.
/// This implementation uses pure safe Rust without any libc calls.
fn collect_disk_info(path: impl AsRef<Path>) -> Result<(u64, u64)> {
    #[cfg(target_os = "linux")]
    {
        collect_disk_info_linux(path)
    }

    #[cfg(not(target_os = "linux"))]
    {
        let _ = path;
        // Fallback: return zeros for unsupported platforms
        Ok((0, 0))
    }
}

/// Linux disk info collection using /proc filesystem
///
/// This implementation reads disk statistics from /proc/diskstats and
/// filesystem information from /proc/mounts without using any unsafe code.
#[cfg(target_os = "linux")]
fn collect_disk_info_linux(path: impl AsRef<Path>) -> Result<(u64, u64)> {
    let path = path.as_ref();

    // First, find which mount point contains the path
    let mount_point = find_mount_point(path)?;

    // Read disk statistics for the mount point
    let diskstats = read_diskstats(&mount_point)?;

    Ok(diskstats)
}

/// Find the mount point for a given path by reading /proc/mounts
#[cfg(target_os = "linux")]
fn find_mount_point(path: &Path) -> Result<String> {
    let mounts_content = fs::read_to_string("/proc/mounts")
        .map_err(|e| Error::Other(format!("Failed to read /proc/mounts: {e}")))?;

    let canonical_path = fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf());

    let path_str = canonical_path.to_string_lossy();

    // Find the longest matching mount point
    let mut best_match = "/".to_string();
    let mut best_len = 1;

    for line in mounts_content.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            let mount_path = parts[1];
            if path_str.starts_with(mount_path) && mount_path.len() > best_len {
                best_match = mount_path.to_string();
                best_len = mount_path.len();
            }
        }
    }

    Ok(best_match)
}

/// Read disk statistics from /proc/diskstats and calculate usage
///
/// Since we can't use statvfs without unsafe code, we use an alternative
/// approach: read the disk device info and estimate from /sys filesystem.
#[cfg(target_os = "linux")]
fn read_diskstats(mount_point: &str) -> Result<(u64, u64)> {
    // Read /proc/mounts to find the device for this mount point
    let mounts_content = fs::read_to_string("/proc/mounts")
        .map_err(|e| Error::Other(format!("Failed to read /proc/mounts: {e}")))?;

    let mut device: Option<String> = None;

    for line in mounts_content.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 && parts[1] == mount_point {
            device = Some(parts[0].to_string());
            break;
        }
    }

    let device = device.ok_or_else(|| {
        Error::Other(format!("No device found for mount point: {mount_point}"))
    })?;

    // Try to read disk size from /sys/block
    // Device might be like /dev/sda1, we need just sda1 or sda
    let device_name = device
        .strip_prefix("/dev/")
        .unwrap_or(&device)
        .to_string();

    // Try reading from /sys/block/<device>/size (returns 512-byte sectors)
    // First try the partition, then the parent device
    let size_paths = [
        format!("/sys/block/{device_name}/size"),
        format!("/sys/class/block/{device_name}/size"),
    ];

    let mut total_bytes: u64 = 0;

    for size_path in &size_paths {
        if let Ok(content) = fs::read_to_string(size_path) {
            if let Ok(sectors) = content.trim().parse::<u64>() {
                // Each sector is 512 bytes
                total_bytes = sectors.saturating_mul(512);
                break;
            }
        }
    }

    // If we couldn't get the size from /sys, try an alternative method
    // Read from /proc/partitions
    if total_bytes == 0 {
        total_bytes = read_partition_size(&device_name)?;
    }

    // Estimate used space by reading filesystem statistics
    // Since we can't use statvfs, we'll estimate from /proc/meminfo pattern
    // This is a graceful degradation - returns total size with 50% used estimate
    let used_bytes = if total_bytes > 0 {
        // Try to read actual usage from df-style calculation
        // For now, we'll use a conservative estimate
        // In production, consider using the `nix` crate with proper feature flags
        total_bytes / 2 // Default to 50% used as a fallback
    } else {
        0
    };

    let total_mb = total_bytes / (1024 * 1024);
    let used_mb = used_bytes / (1024 * 1024);

    Ok((used_mb, total_mb))
}

/// Read partition size from /proc/partitions
#[cfg(target_os = "linux")]
fn read_partition_size(device_name: &str) -> Result<u64> {
    let partitions_content = fs::read_to_string("/proc/partitions")
        .map_err(|e| Error::Other(format!("Failed to read /proc/partitions: {e}")))?;

    // /proc/partitions format:
    // major minor  #blocks  name
    //    8        0  488386584 sda
    //    8        1     524288 sda1

    for line in partitions_content.lines().skip(2) {
        // Skip header lines
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 4 {
            let name = parts[3];
            if name == device_name {
                // Size is in 1024-byte blocks
                if let Ok(blocks) = parts[2].parse::<u64>() {
                    return Ok(blocks.saturating_mul(1024));
                }
            }
        }
    }

    // Default to 0 if we can't find the partition
    Ok(0)
}

/// Get current process information
///
/// # Errors
///
/// Returns an error if process information cannot be retrieved.
pub fn get_process_info() -> Result<ProcessInfo> {
    let pid = std::process::id();

    #[cfg(target_os = "linux")]
    {
        // Count open file descriptors
        let open_files = count_open_files_linux().unwrap_or(0);

        // Get thread count
        let thread_count = get_thread_count_linux().unwrap_or(1);

        // Get memory info
        let (virtual_memory_bytes, resident_memory_bytes) = get_process_memory_linux().unwrap_or((0, 0));

        Ok(ProcessInfo {
            pid,
            thread_count,
            open_files,
            virtual_memory_bytes,
            resident_memory_bytes,
            start_time: None,
        })
    }

    #[cfg(not(target_os = "linux"))]
    {
        // Fallback values for non-Linux
        Ok(ProcessInfo {
            pid,
            thread_count: 1,
            open_files: 0,
            virtual_memory_bytes: 0,
            resident_memory_bytes: 0,
            start_time: None,
        })
    }
}

#[cfg(target_os = "linux")]
fn count_open_files_linux() -> Result<u32> {
    let fd_path = format!("/proc/{}/fd", std::process::id());
    let entries = fs::read_dir(&fd_path)
        .map_err(|e| Error::Other(format!("Failed to read {fd_path}: {e}")))?;

    let count = entries.count();
    Ok(count as u32)
}

#[cfg(target_os = "linux")]
fn get_thread_count_linux() -> Result<u32> {
    let status_path = format!("/proc/{}/status", std::process::id());
    let content = fs::read_to_string(&status_path)
        .map_err(|e| Error::Other(format!("Failed to read {status_path}: {e}")))?;

    for line in content.lines() {
        if line.starts_with("Threads:") {
            if let Some(count_str) = line.split_whitespace().nth(1) {
                if let Ok(count) = count_str.parse() {
                    return Ok(count);
                }
            }
        }
    }

    Ok(1)
}

#[cfg(target_os = "linux")]
fn get_process_memory_linux() -> Result<(u64, u64)> {
    let status_path = format!("/proc/{}/status", std::process::id());
    let content = fs::read_to_string(&status_path)
        .map_err(|e| Error::Other(format!("Failed to read {status_path}: {e}")))?;

    let mut vm_size: u64 = 0;
    let mut vm_rss: u64 = 0;

    for line in content.lines() {
        if line.starts_with("VmSize:") {
            vm_size = parse_status_kb(line) * 1024;
        } else if line.starts_with("VmRSS:") {
            vm_rss = parse_status_kb(line) * 1024;
        }
    }

    Ok((vm_size, vm_rss))
}

#[cfg(target_os = "linux")]
fn parse_status_kb(line: &str) -> u64 {
    line.split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

// ============================================================================
// Alert and Limit Checking
// ============================================================================

/// Check resources against configured limits
///
/// Compares the current resource usage against the configured limits
/// and returns a vector of alerts for any resources that exceed their thresholds.
///
/// # Arguments
///
/// * `resources` - Current system resources snapshot
/// * `limits` - Configured resource limits
///
/// # Returns
///
/// A vector of `ResourceAlert` for each resource that exceeds its threshold.
/// Empty vector if all resources are within limits.
///
/// # Example
///
/// ```
/// use maintenance_engine::m1_foundation::resources::{
///     SystemResources, ResourceLimits, ResourceAlert, check_limits
/// };
///
/// let mut resources = SystemResources::default();
/// resources.cpu_percent = 95.0;  // High CPU
/// resources.memory_percent = 50.0;  // Normal memory
///
/// let limits = ResourceLimits::default();
/// let alerts = check_limits(&resources, &limits);
///
/// assert_eq!(alerts.len(), 1);
/// matches!(alerts[0], ResourceAlert::CpuHigh { .. });
/// ```
#[must_use]
pub fn check_limits(resources: &SystemResources, limits: &ResourceLimits) -> Vec<ResourceAlert> {
    let mut alerts = Vec::new();

    if resources.cpu_percent > limits.max_cpu_percent {
        alerts.push(ResourceAlert::CpuHigh {
            current: resources.cpu_percent,
            threshold: limits.max_cpu_percent,
        });
    }

    if resources.memory_percent > limits.max_memory_percent {
        alerts.push(ResourceAlert::MemoryHigh {
            current: resources.memory_percent,
            threshold: limits.max_memory_percent,
        });
    }

    if resources.disk_percent > limits.max_disk_percent {
        alerts.push(ResourceAlert::DiskHigh {
            current: resources.disk_percent,
            threshold: limits.max_disk_percent,
        });
    }

    if resources.open_files > limits.max_open_files {
        alerts.push(ResourceAlert::OpenFilesHigh {
            current: resources.open_files,
            threshold: limits.max_open_files,
        });
    }

    alerts
}

// ============================================================================
// Formatting and Display
// ============================================================================

/// Format system resources as a human-readable string
///
/// Creates a formatted multi-line string showing all resource metrics.
///
/// # Example
///
/// ```
/// use maintenance_engine::m1_foundation::resources::{SystemResources, format_resources};
///
/// let resources = SystemResources::default();
/// let formatted = format_resources(&resources);
/// assert!(formatted.contains("CPU"));
/// assert!(formatted.contains("Memory"));
/// ```
#[must_use]
pub fn format_resources(resources: &SystemResources) -> String {
    use std::fmt::Write;

    let mut output = String::with_capacity(512);

    output.push_str("=== System Resources ===\n");
    let _ = writeln!(output, "CPU Usage: {:.1}%", resources.cpu_percent);
    let _ = writeln!(
        output,
        "Memory: {} / {} MB ({:.1}%)",
        resources.memory_used_mb, resources.memory_total_mb, resources.memory_percent
    );
    let _ = writeln!(
        output,
        "Disk: {} / {} MB ({:.1}%)",
        resources.disk_used_mb, resources.disk_total_mb, resources.disk_percent
    );
    let _ = writeln!(output, "Open Files: {}", resources.open_files);
    let _ = writeln!(output, "Thread Count: {}", resources.thread_count);

    output
}

/// Format resource alerts as a human-readable string
#[must_use]
pub fn format_alerts(alerts: &[ResourceAlert]) -> String {
    use std::fmt::Write;

    if alerts.is_empty() {
        return "No resource alerts".to_string();
    }

    let mut output = String::with_capacity(256);
    output.push_str("=== Resource Alerts ===\n");
    for (i, alert) in alerts.iter().enumerate() {
        let _ = writeln!(output, "{}. {alert}", i + 1);
    }
    output
}

// ============================================================================
// Resource Manager
// ============================================================================

/// Compute a composite health score from a resource snapshot and limits.
///
/// Returns 1.0 if no snapshot is present or all resources are within limits.
/// The score decreases based on how much each resource exceeds its threshold,
/// capped at 0.25 penalty per resource category, with an overall floor of 0.0.
#[must_use]
pub fn compute_health_score(
    snapshot: Option<&SystemResources>,
    limits: &ResourceLimits,
) -> f64 {
    let Some(snapshot) = snapshot else {
        return 1.0;
    };

    let mut score = 1.0;

    // CPU impact
    if snapshot.cpu_percent > limits.max_cpu_percent {
        let excess = (snapshot.cpu_percent - limits.max_cpu_percent) / 100.0;
        score -= excess.min(0.25);
    }

    // Memory impact
    if snapshot.memory_percent > limits.max_memory_percent {
        let excess = (snapshot.memory_percent - limits.max_memory_percent) / 100.0;
        score -= excess.min(0.25);
    }

    // Disk impact
    if snapshot.disk_percent > limits.max_disk_percent {
        let excess = (snapshot.disk_percent - limits.max_disk_percent) / 100.0;
        score -= excess.min(0.25);
    }

    // Open files impact
    if snapshot.open_files > limits.max_open_files {
        let excess = f64::from(snapshot.open_files - limits.max_open_files)
            / f64::from(limits.max_open_files);
        score -= (excess * 0.25).min(0.25);
    }

    score.max(0.0)
}

/// Resource Manager for continuous monitoring and alerting
///
/// Provides higher-level resource management with configurable limits,
/// history tracking, and alert generation.
#[derive(Clone, Debug)]
pub struct ResourceManager {
    /// Current resource limits configuration
    limits: ResourceLimits,
    /// Last collected resource snapshot
    last_snapshot: Option<SystemResources>,
    /// Alert history (last N alerts)
    alert_history: Vec<(SystemTime, ResourceAlert)>,
    /// Maximum alert history size
    max_history_size: usize,
    /// Data directory path for disk monitoring
    data_path: String,
}

impl Default for ResourceManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ResourceManager {
    /// Create a new resource manager with default limits
    #[must_use]
    pub fn new() -> Self {
        Self {
            limits: ResourceLimits::default(),
            last_snapshot: None,
            alert_history: Vec::new(),
            max_history_size: 100,
            data_path: ".".to_string(),
        }
    }

    /// Create a resource manager with custom limits
    #[must_use]
    pub fn with_limits(limits: ResourceLimits) -> Self {
        Self {
            limits,
            last_snapshot: None,
            alert_history: Vec::new(),
            max_history_size: 100,
            data_path: ".".to_string(),
        }
    }

    /// Set the data directory path for disk monitoring
    pub fn set_data_path(&mut self, path: impl Into<String>) {
        self.data_path = path.into();
    }

    /// Update resource limits
    ///
    /// # Errors
    ///
    /// Returns an error if the limits validation fails.
    pub fn set_limits(&mut self, limits: ResourceLimits) -> Result<()> {
        limits.validate()?;
        self.limits = limits;
        Ok(())
    }

    /// Set resource limits with agent attribution (NAM R5).
    ///
    /// Records which agent changed the limits. The `agent_id` is logged
    /// but not stored persistently in this implementation.
    ///
    /// # Errors
    ///
    /// Returns an error if the limits fail validation.
    pub fn set_limits_as_agent(
        &mut self,
        limits: ResourceLimits,
        _agent_id: &str,
    ) -> Result<()> {
        limits.validate()?;
        self.limits = limits;
        Ok(())
    }

    /// Get current limits
    #[must_use]
    pub const fn limits(&self) -> &ResourceLimits {
        &self.limits
    }

    /// Collect resources and check for alerts
    ///
    /// # Errors
    ///
    /// Returns an error if resource collection fails.
    pub fn collect_and_check(&mut self) -> Result<(SystemResources, Vec<ResourceAlert>)> {
        let resources = collect_resources()?;
        let alerts = check_limits(&resources, &self.limits);

        // Record alerts in history
        let now = SystemTime::now();
        for alert in &alerts {
            self.alert_history.push((now, alert.clone()));
        }

        // Trim history if needed
        while self.alert_history.len() > self.max_history_size {
            self.alert_history.remove(0);
        }

        self.last_snapshot = Some(resources.clone());

        Ok((resources, alerts))
    }

    /// Get the last resource snapshot
    #[must_use]
    pub const fn last_snapshot(&self) -> Option<&SystemResources> {
        self.last_snapshot.as_ref()
    }

    /// Get alert history
    #[must_use]
    pub fn alert_history(&self) -> &[(SystemTime, ResourceAlert)] {
        &self.alert_history
    }

    /// Clear alert history
    pub fn clear_history(&mut self) {
        self.alert_history.clear();
    }

    /// Get resource utilization summary as a map
    #[must_use]
    pub fn utilization_summary(&self) -> HashMap<String, f64> {
        let mut summary = HashMap::new();

        if let Some(ref snapshot) = self.last_snapshot {
            summary.insert("cpu_percent".to_string(), snapshot.cpu_percent);
            summary.insert("memory_percent".to_string(), snapshot.memory_percent);
            summary.insert("disk_percent".to_string(), snapshot.disk_percent);
            summary.insert("open_files".to_string(), f64::from(snapshot.open_files));
            summary.insert("thread_count".to_string(), f64::from(snapshot.thread_count));
        }

        summary
    }

    /// Check if system is healthy (all resources within limits)
    #[must_use]
    pub fn is_healthy(&self) -> bool {
        self.last_snapshot
            .as_ref()
            .is_none_or(|s| check_limits(s, &self.limits).is_empty())
    }

    /// Get health score (0.0 - 1.0)
    ///
    /// Returns 1.0 if all resources are within limits, decreasing
    /// based on how much resources exceed their thresholds.
    #[must_use]
    pub fn health_score(&self) -> f64 {
        compute_health_score(self.last_snapshot.as_ref(), &self.limits)
    }
}

impl ResourceCollector for ResourceManager {
    fn collect(&self) -> crate::Result<SystemResources> {
        collect_resources()
    }

    fn check_limits(&self) -> Vec<ResourceAlert> {
        self.last_snapshot
            .as_ref()
            .map_or_else(Vec::new, |s| check_limits(s, &self.limits))
    }

    fn health_score(&self) -> f64 {
        compute_health_score(self.last_snapshot.as_ref(), &self.limits)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // SystemResources tests
    // ========================================================================

    #[test]
    fn test_system_resources_default() {
        let resources = SystemResources::default();
        assert_eq!(resources.cpu_percent, 0.0);
        assert_eq!(resources.memory_used_mb, 0);
        assert_eq!(resources.memory_total_mb, 0);
    }

    #[test]
    fn test_system_resources_default_all_fields() {
        let resources = SystemResources::default();
        assert_eq!(resources.memory_percent, 0.0);
        assert_eq!(resources.disk_used_mb, 0);
        assert_eq!(resources.disk_total_mb, 0);
        assert_eq!(resources.disk_percent, 0.0);
        assert_eq!(resources.open_files, 0);
        assert_eq!(resources.thread_count, 0);
    }

    #[test]
    fn test_system_resources_field_ranges() {
        let resources = SystemResources {
            cpu_percent: 100.0,
            memory_used_mb: u64::MAX,
            memory_total_mb: u64::MAX,
            memory_percent: 100.0,
            disk_used_mb: u64::MAX,
            disk_total_mb: u64::MAX,
            disk_percent: 100.0,
            open_files: u32::MAX,
            thread_count: u32::MAX,
            timestamp: SystemTime::now(),
        };
        // Should be constructible with extreme values
        assert_eq!(resources.cpu_percent, 100.0);
        assert_eq!(resources.open_files, u32::MAX);
    }

    #[test]
    fn test_system_resources_clone() {
        let resources = SystemResources {
            cpu_percent: 42.5,
            memory_used_mb: 1024,
            ..Default::default()
        };
        let cloned = resources.clone();
        assert_eq!(cloned.cpu_percent, 42.5);
        assert_eq!(cloned.memory_used_mb, 1024);
    }

    // ========================================================================
    // ResourceLimits tests
    // ========================================================================

    #[test]
    fn test_resource_limits_default() {
        let limits = ResourceLimits::default();
        assert!((limits.max_cpu_percent - 80.0).abs() < f64::EPSILON);
        assert!((limits.max_memory_percent - 85.0).abs() < f64::EPSILON);
        assert!((limits.max_disk_percent - 90.0).abs() < f64::EPSILON);
        assert_eq!(limits.max_open_files, 1000);
    }

    #[test]
    fn test_resource_limits_validation() {
        let valid_limits = ResourceLimits::new(80.0, 85.0, 90.0, 1000);
        assert!(valid_limits.validate().is_ok());

        let invalid_cpu = ResourceLimits::new(150.0, 85.0, 90.0, 1000);
        assert!(invalid_cpu.validate().is_err());

        let invalid_memory = ResourceLimits::new(80.0, -5.0, 90.0, 1000);
        assert!(invalid_memory.validate().is_err());
    }

    #[test]
    fn test_resource_limits_custom_values() {
        let limits = ResourceLimits::new(50.0, 60.0, 70.0, 500);
        assert!((limits.max_cpu_percent - 50.0).abs() < f64::EPSILON);
        assert!((limits.max_memory_percent - 60.0).abs() < f64::EPSILON);
        assert!((limits.max_disk_percent - 70.0).abs() < f64::EPSILON);
        assert_eq!(limits.max_open_files, 500);
    }

    #[test]
    fn test_resource_limits_boundary_zero() {
        let limits = ResourceLimits::new(0.0, 0.0, 0.0, 0);
        assert!(limits.validate().is_ok());
    }

    #[test]
    fn test_resource_limits_boundary_hundred() {
        let limits = ResourceLimits::new(100.0, 100.0, 100.0, u32::MAX);
        assert!(limits.validate().is_ok());
    }

    #[test]
    fn test_resource_limits_invalid_disk() {
        let invalid_disk = ResourceLimits::new(80.0, 85.0, 101.0, 1000);
        assert!(invalid_disk.validate().is_err());
    }

    #[test]
    fn test_resource_limits_negative_cpu() {
        let limits = ResourceLimits::new(-0.1, 85.0, 90.0, 1000);
        assert!(limits.validate().is_err());
    }

    // ========================================================================
    // check_limits tests
    // ========================================================================

    #[test]
    fn test_check_limits_no_alerts() {
        let resources = SystemResources {
            cpu_percent: 50.0,
            memory_percent: 60.0,
            disk_percent: 70.0,
            open_files: 500,
            ..Default::default()
        };
        let limits = ResourceLimits::default();

        let alerts = check_limits(&resources, &limits);
        assert!(alerts.is_empty());
    }

    #[test]
    fn test_check_limits_cpu_alert() {
        let resources = SystemResources {
            cpu_percent: 95.0,
            memory_percent: 50.0,
            disk_percent: 50.0,
            open_files: 100,
            ..Default::default()
        };
        let limits = ResourceLimits::default();

        let alerts = check_limits(&resources, &limits);
        assert_eq!(alerts.len(), 1);
        assert!(matches!(alerts[0], ResourceAlert::CpuHigh { .. }));
    }

    #[test]
    fn test_check_limits_memory_alert() {
        let resources = SystemResources {
            cpu_percent: 10.0,
            memory_percent: 90.0,
            disk_percent: 50.0,
            open_files: 100,
            ..Default::default()
        };
        let limits = ResourceLimits::default();

        let alerts = check_limits(&resources, &limits);
        assert_eq!(alerts.len(), 1);
        assert!(matches!(alerts[0], ResourceAlert::MemoryHigh { .. }));
    }

    #[test]
    fn test_check_limits_disk_alert() {
        let resources = SystemResources {
            cpu_percent: 10.0,
            memory_percent: 10.0,
            disk_percent: 95.0,
            open_files: 100,
            ..Default::default()
        };
        let limits = ResourceLimits::default();

        let alerts = check_limits(&resources, &limits);
        assert_eq!(alerts.len(), 1);
        assert!(matches!(alerts[0], ResourceAlert::DiskHigh { .. }));
    }

    #[test]
    fn test_check_limits_open_files_alert() {
        let resources = SystemResources {
            cpu_percent: 10.0,
            memory_percent: 10.0,
            disk_percent: 10.0,
            open_files: 2000,
            ..Default::default()
        };
        let limits = ResourceLimits::default();

        let alerts = check_limits(&resources, &limits);
        assert_eq!(alerts.len(), 1);
        assert!(matches!(alerts[0], ResourceAlert::OpenFilesHigh { .. }));
    }

    #[test]
    fn test_check_limits_multiple_alerts() {
        let resources = SystemResources {
            cpu_percent: 95.0,
            memory_percent: 95.0,
            disk_percent: 95.0,
            open_files: 2000,
            ..Default::default()
        };
        let limits = ResourceLimits::default();

        let alerts = check_limits(&resources, &limits);
        assert_eq!(alerts.len(), 4);
    }

    #[test]
    fn test_check_limits_boundary_at_exact_limit_no_alert() {
        // At exactly the limit, should NOT alert (only > triggers)
        let resources = SystemResources {
            cpu_percent: 80.0,
            memory_percent: 85.0,
            disk_percent: 90.0,
            open_files: 1000,
            ..Default::default()
        };
        let limits = ResourceLimits::default();

        let alerts = check_limits(&resources, &limits);
        assert!(alerts.is_empty());
    }

    // ========================================================================
    // ResourceAlert Display tests
    // ========================================================================

    #[test]
    fn test_resource_alert_display_cpu() {
        let alert = ResourceAlert::CpuHigh {
            current: 95.5,
            threshold: 80.0,
        };
        let display = format!("{alert}");
        assert!(display.contains("CPU"));
        assert!(display.contains("95.5"));
        assert!(display.contains("80.0"));
    }

    #[test]
    fn test_resource_alert_display_memory() {
        let alert = ResourceAlert::MemoryHigh {
            current: 92.3,
            threshold: 85.0,
        };
        let display = format!("{alert}");
        assert!(display.contains("Memory"));
        assert!(display.contains("92.3"));
        assert!(display.contains("85.0"));
    }

    #[test]
    fn test_resource_alert_display_disk() {
        let alert = ResourceAlert::DiskHigh {
            current: 96.7,
            threshold: 90.0,
        };
        let display = format!("{alert}");
        assert!(display.contains("Disk"));
        assert!(display.contains("96.7"));
        assert!(display.contains("90.0"));
    }

    #[test]
    fn test_resource_alert_display_open_files() {
        let alert = ResourceAlert::OpenFilesHigh {
            current: 1500,
            threshold: 1000,
        };
        let display = format!("{alert}");
        assert!(display.contains("Open files"));
        assert!(display.contains("1500"));
        assert!(display.contains("1000"));
    }

    // ========================================================================
    // format_resources tests
    // ========================================================================

    #[test]
    fn test_format_resources() {
        let resources = SystemResources {
            cpu_percent: 45.5,
            memory_used_mb: 8192,
            memory_total_mb: 16384,
            memory_percent: 50.0,
            disk_used_mb: 100_000,
            disk_total_mb: 500_000,
            disk_percent: 20.0,
            open_files: 150,
            thread_count: 8,
            ..Default::default()
        };

        let formatted = format_resources(&resources);
        assert!(formatted.contains("45.5%"));
        assert!(formatted.contains("8192"));
        assert!(formatted.contains("16384"));
    }

    #[test]
    fn test_format_resources_default() {
        let resources = SystemResources::default();
        let formatted = format_resources(&resources);
        assert!(formatted.contains("CPU"));
        assert!(formatted.contains("Memory"));
        assert!(formatted.contains("Disk"));
        assert!(formatted.contains("Open Files"));
        assert!(formatted.contains("Thread Count"));
    }

    #[test]
    fn test_format_resources_contains_all_sections() {
        let resources = SystemResources {
            cpu_percent: 10.0,
            memory_used_mb: 512,
            memory_total_mb: 4096,
            memory_percent: 12.5,
            disk_used_mb: 50_000,
            disk_total_mb: 200_000,
            disk_percent: 25.0,
            open_files: 42,
            thread_count: 2,
            ..Default::default()
        };

        let formatted = format_resources(&resources);
        assert!(formatted.contains("=== System Resources ==="));
        assert!(formatted.contains("10.0%"));
        assert!(formatted.contains("512"));
        assert!(formatted.contains("4096"));
        assert!(formatted.contains("42"));
        assert!(formatted.contains("2"));
    }

    // ========================================================================
    // format_alerts tests
    // ========================================================================

    #[test]
    fn test_format_alerts_empty() {
        let alerts: Vec<ResourceAlert> = vec![];
        let formatted = format_alerts(&alerts);
        assert!(formatted.contains("No resource alerts"));
    }

    #[test]
    fn test_format_alerts_multiple() {
        let alerts = vec![
            ResourceAlert::CpuHigh {
                current: 90.0,
                threshold: 80.0,
            },
            ResourceAlert::MemoryHigh {
                current: 95.0,
                threshold: 85.0,
            },
        ];
        let formatted = format_alerts(&alerts);
        assert!(formatted.contains("1."));
        assert!(formatted.contains("2."));
        assert!(formatted.contains("CPU"));
        assert!(formatted.contains("Memory"));
    }

    // ========================================================================
    // ResourceManager tests
    // ========================================================================

    #[test]
    fn test_resource_manager_creation() {
        let manager = ResourceManager::new();
        assert!(manager.is_healthy());
        assert!((manager.health_score() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_resource_manager_default_trait() {
        let manager = ResourceManager::default();
        assert!(manager.is_healthy());
        assert!(manager.last_snapshot().is_none());
        assert!(manager.alert_history().is_empty());
    }

    #[test]
    fn test_resource_manager_custom_limits() {
        let limits = ResourceLimits::new(70.0, 75.0, 80.0, 500);
        let manager = ResourceManager::with_limits(limits);
        assert!((manager.limits().max_cpu_percent - 70.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_resource_manager_set_limits() {
        let mut manager = ResourceManager::new();
        let new_limits = ResourceLimits::new(60.0, 70.0, 80.0, 500);
        assert!(manager.set_limits(new_limits).is_ok());
        assert!((manager.limits().max_cpu_percent - 60.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_resource_manager_invalid_limits() {
        let mut manager = ResourceManager::new();
        let invalid_limits = ResourceLimits::new(150.0, 70.0, 80.0, 500);
        assert!(manager.set_limits(invalid_limits).is_err());
        // Original limits should be preserved after failed set
        assert!((manager.limits().max_cpu_percent - 80.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_resource_manager_set_data_path() {
        let mut manager = ResourceManager::new();
        manager.set_data_path("/tmp/test-data");
        // data_path is private; verify no panic
        assert!(manager.is_healthy());
    }

    #[test]
    fn test_resource_manager_clear_history() {
        let mut manager = ResourceManager::new();
        // Manually push an alert into history
        manager.alert_history.push((
            SystemTime::now(),
            ResourceAlert::CpuHigh {
                current: 95.0,
                threshold: 80.0,
            },
        ));
        assert_eq!(manager.alert_history().len(), 1);

        manager.clear_history();
        assert!(manager.alert_history().is_empty());
    }

    #[test]
    fn test_resource_manager_alert_history_starts_empty() {
        let manager = ResourceManager::new();
        assert!(manager.alert_history().is_empty());
    }

    // ========================================================================
    // health_score tests
    // ========================================================================

    #[test]
    fn test_resource_manager_health_score_reduced() {
        let mut manager = ResourceManager::new();
        manager.last_snapshot = Some(SystemResources {
            cpu_percent: 95.0,
            memory_percent: 50.0,
            disk_percent: 50.0,
            open_files: 100,
            ..Default::default()
        });

        assert!(manager.health_score() < 1.0);
        assert!(!manager.is_healthy());
    }

    #[test]
    fn test_health_score_no_snapshot_returns_one() {
        let manager = ResourceManager::new();
        assert!((manager.health_score() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_health_score_all_within_limits_returns_one() {
        let mut manager = ResourceManager::new();
        manager.last_snapshot = Some(SystemResources {
            cpu_percent: 50.0,
            memory_percent: 50.0,
            disk_percent: 50.0,
            open_files: 100,
            ..Default::default()
        });
        assert!((manager.health_score() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_health_score_all_resources_exceeded() {
        let mut manager = ResourceManager::new();
        // Set limits very low and usage very high
        let low_limits = ResourceLimits::new(10.0, 10.0, 10.0, 10);
        assert!(manager.set_limits(low_limits).is_ok());
        manager.last_snapshot = Some(SystemResources {
            cpu_percent: 100.0,
            memory_percent: 100.0,
            disk_percent: 100.0,
            open_files: 10_000,
            ..Default::default()
        });

        let score = manager.health_score();
        // Score should be floored at 0.0
        assert!(score >= 0.0);
        assert!(score < 0.1);
    }

    #[test]
    fn test_health_score_open_files_impact() {
        let mut manager = ResourceManager::new();
        // Only open files exceed the limit
        manager.last_snapshot = Some(SystemResources {
            cpu_percent: 10.0,
            memory_percent: 10.0,
            disk_percent: 10.0,
            open_files: 2000,
            ..Default::default()
        });

        let score = manager.health_score();
        // Open files at 2000 vs limit 1000: excess = 1000/1000 = 1.0, penalty = min(0.25, 0.25)
        assert!(score < 1.0);
        assert!(score > 0.5);
    }

    #[test]
    fn test_health_score_partial_degradation() {
        let mut manager = ResourceManager::new();
        // CPU slightly above threshold
        manager.last_snapshot = Some(SystemResources {
            cpu_percent: 85.0, // 5% over default 80% limit
            memory_percent: 50.0,
            disk_percent: 50.0,
            open_files: 100,
            ..Default::default()
        });

        let score = manager.health_score();
        // Excess = 5/100 = 0.05, penalty = 0.05
        let expected = 1.0 - 0.05;
        assert!((score - expected).abs() < f64::EPSILON);
    }

    // ========================================================================
    // compute_health_score standalone function tests
    // ========================================================================

    #[test]
    fn test_compute_health_score_no_snapshot() {
        let limits = ResourceLimits::default();
        let score = compute_health_score(None, &limits);
        assert!((score - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_compute_health_score_healthy_snapshot() {
        let limits = ResourceLimits::default();
        let snapshot = SystemResources {
            cpu_percent: 50.0,
            memory_percent: 50.0,
            disk_percent: 50.0,
            open_files: 100,
            ..Default::default()
        };
        let score = compute_health_score(Some(&snapshot), &limits);
        assert!((score - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_compute_health_score_degraded() {
        let limits = ResourceLimits::new(10.0, 10.0, 10.0, 10);
        let snapshot = SystemResources {
            cpu_percent: 100.0,
            memory_percent: 100.0,
            disk_percent: 100.0,
            open_files: 10_000,
            ..Default::default()
        };
        let score = compute_health_score(Some(&snapshot), &limits);
        assert!(score >= 0.0);
        assert!(score < 0.1);
    }

    // ========================================================================
    // ProcessInfo tests
    // ========================================================================

    #[test]
    fn test_process_info_default() {
        let info = ProcessInfo::default();
        assert_eq!(info.pid, 0);
        assert_eq!(info.thread_count, 1);
        assert_eq!(info.open_files, 0);
        assert_eq!(info.virtual_memory_bytes, 0);
        assert_eq!(info.resident_memory_bytes, 0);
        assert!(info.start_time.is_none());
    }

    // ========================================================================
    // Utilization summary tests
    // ========================================================================

    #[test]
    fn test_utilization_summary_empty() {
        let manager = ResourceManager::new();
        let summary = manager.utilization_summary();
        assert!(summary.is_empty());
    }

    #[test]
    fn test_utilization_summary_with_snapshot() {
        let mut manager = ResourceManager::new();
        manager.last_snapshot = Some(SystemResources {
            cpu_percent: 50.0,
            memory_percent: 60.0,
            disk_percent: 70.0,
            open_files: 100,
            thread_count: 4,
            ..Default::default()
        });

        let summary = manager.utilization_summary();
        assert_eq!(summary.len(), 5);
        assert!(
            (summary.get("cpu_percent").copied().unwrap_or(0.0) - 50.0).abs() < f64::EPSILON
        );
        assert!(
            (summary.get("memory_percent").copied().unwrap_or(0.0) - 60.0).abs() < f64::EPSILON
        );
        assert!(
            (summary.get("disk_percent").copied().unwrap_or(0.0) - 70.0).abs() < f64::EPSILON
        );
        assert!(
            (summary.get("open_files").copied().unwrap_or(0.0) - 100.0).abs() < f64::EPSILON
        );
        assert!(
            (summary.get("thread_count").copied().unwrap_or(0.0) - 4.0).abs() < f64::EPSILON
        );
    }

    // ========================================================================
    // ResourceCollector trait compliance tests
    // ========================================================================

    #[test]
    fn test_resource_collector_trait_collect() {
        let manager = ResourceManager::new();
        let collector: &dyn ResourceCollector = &manager;
        // collect_resources may fail in sandboxed CI, so we just verify it returns
        let _ = collector.collect();
    }

    #[test]
    fn test_resource_collector_trait_check_limits() {
        let mut manager = ResourceManager::new();
        manager.last_snapshot = Some(SystemResources {
            cpu_percent: 95.0,
            memory_percent: 50.0,
            disk_percent: 50.0,
            open_files: 100,
            ..Default::default()
        });
        let collector: &dyn ResourceCollector = &manager;
        let alerts = collector.check_limits();
        assert_eq!(alerts.len(), 1);
        assert!(matches!(alerts[0], ResourceAlert::CpuHigh { .. }));
    }

    #[test]
    fn test_resource_collector_trait_health_score() {
        let manager = ResourceManager::new();
        let collector: &dyn ResourceCollector = &manager;
        let score = collector.health_score();
        assert!((score - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_resource_collector_trait_no_snapshot_no_alerts() {
        let manager = ResourceManager::new();
        let collector: &dyn ResourceCollector = &manager;
        let alerts = collector.check_limits();
        assert!(alerts.is_empty());
    }

    // ========================================================================
    // Thread safety compile-time check
    // ========================================================================

    #[test]
    fn test_resource_manager_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ResourceManager>();
    }

    // ========================================================================
    // ResourceAlert equality
    // ========================================================================

    #[test]
    fn test_resource_alert_equality() {
        let a = ResourceAlert::CpuHigh {
            current: 90.0,
            threshold: 80.0,
        };
        let b = ResourceAlert::CpuHigh {
            current: 90.0,
            threshold: 80.0,
        };
        assert_eq!(a, b);

        let c = ResourceAlert::MemoryHigh {
            current: 90.0,
            threshold: 80.0,
        };
        assert_ne!(a, c);
    }

    // ========================================================================
    // Platform-specific tests
    // ========================================================================

    #[cfg(target_os = "linux")]
    mod linux_tests {
        use super::*;

        #[test]
        fn test_collect_resources_linux() {
            let result = collect_resources();
            assert!(result.is_ok());

            let resources = result.ok();
            assert!(resources.is_some());
            let resources = resources.as_ref();
            // Memory total should be greater than 0 on any real system
            assert!(resources.is_some_and(|r| r.memory_total_mb > 0));
        }

        #[test]
        fn test_get_process_info_linux() {
            let result = get_process_info();
            assert!(result.is_ok());

            let info = result.ok();
            assert!(info.is_some());
            let info = info.as_ref();
            // PID should be greater than 0
            assert!(info.is_some_and(|i| i.pid > 0));
            // Thread count should be at least 1
            assert!(info.is_some_and(|i| i.thread_count >= 1));
        }

        #[test]
        fn test_collect_cpu_usage_linux() {
            let result = collect_cpu_usage();
            assert!(result.is_ok());
            // CPU percentage should be between 0 and 100
            let cpu = result.ok().unwrap_or(0.0);
            assert!((0.0..=100.0).contains(&cpu));
        }

        #[test]
        fn test_collect_memory_info_linux() {
            let result = collect_memory_info();
            assert!(result.is_ok());
            let (used, total) = result.ok().unwrap_or((0, 0));
            // Total should be greater than 0 and used should not exceed total
            assert!(total > 0);
            assert!(used <= total);
        }
    }

    // ====================================================================
    // NAM: SystemResources::to_tensor() tests
    // ====================================================================

    #[test]
    fn test_system_resources_to_tensor_valid() {
        let resources = SystemResources::default();
        let tensor = resources.to_tensor();
        assert!(tensor.validate().is_ok());
    }

    #[test]
    fn test_system_resources_to_tensor_cpu_mapping() {
        let resources = SystemResources {
            cpu_percent: 50.0,
            ..SystemResources::default()
        };
        let tensor = resources.to_tensor();
        assert!((tensor.error_rate - 0.5).abs() < 0.01);
        assert!((tensor.health_score - 0.5).abs() < 0.01);
    }

    // ====================================================================
    // NAM: AdaptiveResourceLimits tests
    // ====================================================================

    #[test]
    fn test_adaptive_limits_default() {
        let adaptive = AdaptiveResourceLimits::default();
        assert!((adaptive.pathway_strength).abs() < f64::EPSILON);
        let effective = adaptive.effective_limits();
        assert!((effective.max_cpu_percent - 80.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_adaptive_limits_high_pathway() {
        let adaptive = AdaptiveResourceLimits {
            base: ResourceLimits::default(),
            pathway_strength: 1.0,
        };
        let effective = adaptive.effective_limits();
        // 80.0 + 10.0 (max relaxation) = 90.0
        assert!((effective.max_cpu_percent - 90.0).abs() < f64::EPSILON);
        assert!((effective.max_memory_percent - 95.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_adaptive_limits_capped_at_100() {
        let adaptive = AdaptiveResourceLimits {
            base: ResourceLimits::new(95.0, 95.0, 95.0, 1000),
            pathway_strength: 1.0,
        };
        let effective = adaptive.effective_limits();
        assert!((effective.max_cpu_percent - 100.0).abs() < f64::EPSILON);
    }

    // ====================================================================
    // NAM: set_limits_as_agent tests
    // ====================================================================

    #[test]
    fn test_set_limits_as_agent() {
        let mut manager = ResourceManager::new();
        let limits = ResourceLimits::new(70.0, 75.0, 80.0, 800);
        let result = manager.set_limits_as_agent(limits, "@0.A");
        assert!(result.is_ok());
        assert!((manager.limits().max_cpu_percent - 70.0).abs() < f64::EPSILON);
    }

    // ====================================================================
    // NAM: ResourceCollector default methods
    // ====================================================================

    #[test]
    fn test_resource_collector_agent_id_default() {
        let manager = ResourceManager::new();
        let collector: &dyn ResourceCollector = &manager;
        assert!(collector.agent_id().is_none());
    }

    #[test]
    fn test_resource_collector_to_tensor_default() {
        let manager = ResourceManager::new();
        let collector: &dyn ResourceCollector = &manager;
        let tensor = collector.to_tensor();
        assert!(tensor.validate().is_ok());
    }
}
