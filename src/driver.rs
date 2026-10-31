//! NVMe Character Device IOCTL & Admin Worker
//!
//! Submits raw `NVME_ADMIN_GET_LOG_PAGE` commands directly to character devices
//! like `/dev/nvme0`. Requires `CAP_SYS_ADMIN` or `CAP_SYS_RAWIO` permissions.
//! Falls back gracefully to kernel synthetic simulation if running in unprivileged test mode.

use crate::nvme_spec::{
    NvmeAdminCmd, NvmeSmartLog, NvmeSmartMetrics, NVME_ADMIN_GET_LOG_PAGE,
    NVME_IOCTL_ADMIN_CMD, NVME_LOG_LID_SMART_HEALTH,
};
use std::fs::OpenOptions;
use std::io;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;

pub struct NvmeDriver;

impl NvmeDriver {
    /// Queries the 512-byte SMART / Health Log Page from the specified NVMe character device.
    /// Example: `device_path = "/dev/nvme0"`
    pub fn read_smart_log(device_path: &str) -> io::Result<NvmeSmartMetrics> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(device_path)?;

        let mut smart_log = NvmeSmartLog::default();

        // Prepare NVMe Admin Command: Get Log Page
        // cdw10: (numdl << 16) | lid
        // numdl = (512 / 4) - 1 = 127 = 0x007F
        // LID = 0x02 (SMART / Health)
        let numdl = (std::mem::size_of::<NvmeSmartLog>() / 4) - 1;
        let cdw10 = ((numdl as u32) << 16) | (NVME_LOG_LID_SMART_HEALTH as u32);

        let mut admin_cmd = NvmeAdminCmd {
            opcode: NVME_ADMIN_GET_LOG_PAGE,
            nsid: 0xFFFFFFFF, // Broadcast to controller / all namespaces
            addr: &mut smart_log as *mut _ as u64,
            data_len: std::mem::size_of::<NvmeSmartLog>() as u32,
            cdw10,
            ..Default::default()
        };

        let fd = file.as_raw_fd();
        let ret = unsafe { libc::ioctl(fd, NVME_IOCTL_ADMIN_CMD, &mut admin_cmd) };

        if ret < 0 {
            return Err(io::Error::last_os_error());
        }

        Ok(NvmeSmartMetrics::from_raw(&smart_log))
    }

    /// Synthesizes realistic hardware telemetry for testing, sandbox environments,
    /// or unprivileged development without root raw I/O privileges.
    pub fn synthesize_metrics(step: u64) -> NvmeSmartMetrics {
        let temp_celsius = 42.0 + (step as f64 % 15.0);
        let percentage_used = 3;
        let available_spare_pct = 100;
        let spare_threshold_pct = 10;
        let data_read_bytes = 4_520_000_000_000u128 + (step as u128 * 1_000_000);
        let data_written_bytes = 3_150_000_000_000u128 + (step as u128 * 800_000);

        NvmeSmartMetrics {
            critical_warning: 0,
            temp_celsius,
            available_spare_pct,
            spare_threshold_pct,
            percentage_used,
            data_read_bytes,
            data_written_bytes,
            host_reads_count: 85_420_000 + step as u128,
            host_writes_count: 62_110_000 + step as u128,
            power_cycles: 412,
            power_on_hours: 1_280,
            unsafe_shutdowns: 2,
            media_errors: 0,
            error_log_entries: 0,
        }
    }
}
