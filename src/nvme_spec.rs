//! NVMe Admin Command & SMART / Health Information Log Page Specifications
//!
//! Compliant with NVM Express Base Specification Revision 2.0 / 1.4.
//! Direct zero-allocation mapping over 512-byte raw binary payloads.

use std::fmt;

/// NVMe Admin Opcodes (NVM Express Base Specification)
pub const NVME_ADMIN_GET_LOG_PAGE: u8 = 0x02;

/// NVMe Log Page Identifiers (LID)
pub const NVME_LOG_LID_ERROR_INFO: u8 = 0x01;
pub const NVME_LOG_LID_SMART_HEALTH: u8 = 0x02;

/// Linux Kernel NVMe Admin Command IOCTL request code.
/// Defined in `<linux/nvme_ioctl.h>` as `_IOWR('N', 0x41, struct nvme_admin_cmd)`
/// On Linux x86_64: sizeof(struct nvme_admin_cmd) = 72 bytes.
/// _IOC(_IOC_READ|_IOC_WRITE, 'N', 0x41, 72) = 0xC0484E41
pub const NVME_IOCTL_ADMIN_CMD: libc::c_ulong = 0xC0484E41;

/// Raw Linux kernel `struct nvme_admin_cmd` layout (72 bytes).
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct NvmeAdminCmd {
    pub opcode: u8,
    pub flags: u8,
    pub rsvd1: u16,
    pub nsid: u32,
    pub cdw2: u32,
    pub cdw3: u32,
    pub metadata: u64,
    pub addr: u64,
    pub metadata_len: u32,
    pub data_len: u32,
    pub cdw10: u32,
    pub cdw11: u32,
    pub cdw12: u32,
    pub cdw13: u32,
    pub cdw14: u32,
    pub cdw15: u32,
    pub timeout_ms: u32,
    pub result: u32,
}

impl Default for NvmeAdminCmd {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

/// NVMe SMART / Health Information Log Page (512 bytes).
/// Exact byte layout per NVMe Base Spec Section 5.14.1.2.
#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct NvmeSmartLog {
    /// Critical Warning bitmask
    pub critical_warning: u8,
    /// Composite Temperature in Kelvin (Little-Endian)
    pub composite_temperature: [u8; 2],
    /// Available Spare percentage (0 - 100)
    pub available_spare: u8,
    /// Available Spare Threshold percentage
    pub available_spare_threshold: u8,
    /// Percentage Used (Estimate of device life consumed, 0 - 255)
    pub percentage_used: u8,
    /// Endurance Group Critical Warnings Summary
    pub endurance_warning: u8,
    pub rsvd7: [u8; 25],
    /// Data Units Read (1 unit = 1000 sectors of 512 bytes = 512,000 bytes) as 128-bit LE
    pub data_units_read: [u8; 16],
    /// Data Units Written (1 unit = 1000 sectors of 512 bytes = 512,000 bytes) as 128-bit LE
    pub data_units_written: [u8; 16],
    /// Host Read Commands (128-bit LE)
    pub host_reads: [u8; 16],
    /// Host Write Commands (128-bit LE)
    pub host_writes: [u8; 16],
    /// Controller Busy Time in minutes (128-bit LE)
    pub controller_busy_time: [u8; 16],
    /// Power Cycles (128-bit LE)
    pub power_cycles: [u8; 16],
    /// Power On Hours (128-bit LE)
    pub power_on_hours: [u8; 16],
    /// Unsafe Shutdowns (128-bit LE)
    pub unsafe_shutdowns: [u8; 16],
    /// Media and Data Integrity Errors (128-bit LE)
    pub media_errors: [u8; 16],
    /// Number of Error Information Log Entries (128-bit LE)
    pub num_err_log_entries: [u8; 16],
    /// Warning Composite Temperature Time in minutes
    pub warning_temp_time: [u8; 4],
    /// Critical Composite Temperature Time in minutes
    pub critical_temp_time: [u8; 4],
    /// Temperature Sensors 1 to 8 in Kelvin
    pub temp_sensors: [u16; 8],
    /// Thermal Management Transition Counts
    pub thermal_transitions: [u32; 2],
    /// Total Time For Thermal Management (seconds)
    pub thermal_time: [u32; 2],
    pub rsvd232: [u8; 280],
}

impl Default for NvmeSmartLog {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

/// Parsed, ergonomic representation of an NVMe SMART snapshot.
#[derive(Debug, Clone)]
pub struct NvmeSmartMetrics {
    pub critical_warning: u8,
    pub temp_celsius: f64,
    pub available_spare_pct: u8,
    pub spare_threshold_pct: u8,
    pub percentage_used: u8,
    pub data_read_bytes: u128,
    pub data_written_bytes: u128,
    pub host_reads_count: u128,
    pub host_writes_count: u128,
    pub power_cycles: u128,
    pub power_on_hours: u128,
    pub unsafe_shutdowns: u128,
    pub media_errors: u128,
    pub error_log_entries: u128,
}

impl NvmeSmartMetrics {
    /// Decodes raw binary 512-byte struct into safe, human-readable metrics.
    pub fn from_raw(raw: &NvmeSmartLog) -> Self {
        let raw_kelvin = u16::from_le_bytes(raw.composite_temperature) as f64;
        let temp_c = if raw_kelvin > 0.0 { raw_kelvin - 273.15 } else { 0.0 };

        let data_read_units = u128::from_le_bytes(raw.data_units_read);
        let data_written_units = u128::from_le_bytes(raw.data_units_written);

        // 1 Data Unit = 1000 sectors of 512 bytes = 512,000 bytes
        let data_read_bytes = data_read_units.saturating_mul(512_000);
        let data_written_bytes = data_written_units.saturating_mul(512_000);

        Self {
            critical_warning: raw.critical_warning,
            temp_celsius: temp_c,
            available_spare_pct: raw.available_spare,
            spare_threshold_pct: raw.available_spare_threshold,
            percentage_used: raw.percentage_used,
            data_read_bytes,
            data_written_bytes,
            host_reads_count: u128::from_le_bytes(raw.host_reads),
            host_writes_count: u128::from_le_bytes(raw.host_writes),
            power_cycles: u128::from_le_bytes(raw.power_cycles),
            power_on_hours: u128::from_le_bytes(raw.power_on_hours),
            unsafe_shutdowns: u128::from_le_bytes(raw.unsafe_shutdowns),
            media_errors: u128::from_le_bytes(raw.media_errors),
            error_log_entries: u128::from_le_bytes(raw.num_err_log_entries),
        }
    }
}

impl fmt::Display for NvmeSmartMetrics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Temp: {:.1}°C | Spare: {}% (Thresh: {}%) | Life Used: {}% | Media Errs: {} | Unsafe Shutdowns: {}",
            self.temp_celsius,
            self.available_spare_pct,
            self.spare_threshold_pct,
            self.percentage_used,
            self.media_errors,
            self.unsafe_shutdowns
        )
    }
}
