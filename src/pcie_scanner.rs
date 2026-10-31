//! Linux Kernel PCIe & sysfs Hardware Interfacing Engine
//!
//! Directly interrogates Linux `/sys/bus/pci` and `/sys/class/nvme` virtual filesystems.
//! Extracts real-time negotiated PCIe link speed, link width, maximum capabilities,
//! and Advanced Error Reporting (AER) states.

use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct PcieLinkState {
    pub pci_address: String,
    pub current_speed: String,
    pub current_width: u32,
    pub max_speed: String,
    pub max_width: u32,
    pub link_degraded: bool,
}

#[derive(Debug, Clone)]
pub struct ControllerIdentity {
    pub name: String,
    pub model: String,
    pub firmware: String,
    pub pci_path: PathBuf,
}

pub struct PcieScanner;

impl PcieScanner {
    /// Detects all active NVMe controllers registered in `/sys/class/nvme/`.
    pub fn discover_controllers() -> Vec<ControllerIdentity> {
        let mut controllers = Vec::new();
        let sys_nvme = Path::new("/sys/class/nvme");

        if let Ok(entries) = fs::read_dir(sys_nvme) {
            for entry in entries.flatten() {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();

                let model = fs::read_to_string(path.join("model"))
                    .unwrap_or_else(|_| "Unknown Model".into())
                    .trim()
                    .to_string();

                let firmware = fs::read_to_string(path.join("firmware_rev"))
                    .unwrap_or_else(|_| "Unknown FW".into())
                    .trim()
                    .to_string();

                let pci_path = path.join("device");

                controllers.push(ControllerIdentity {
                    name,
                    model,
                    firmware,
                    pci_path,
                });
            }
        }

        controllers
    }

    /// Reads negotiated PCIe link speed and width from `/sys/bus/pci/devices/<addr>/`
    pub fn read_pcie_link(pci_path: &Path) -> PcieLinkState {
        let pci_addr = pci_path
            .canonicalize()
            .map(|p| p.file_name().unwrap_or_default().to_string_lossy().to_string())
            .unwrap_or_else(|_| "0000:00:00.0".into());

        let current_speed = fs::read_to_string(pci_path.join("current_link_speed"))
            .unwrap_or_else(|_| "Unknown".into())
            .trim()
            .to_string();

        let current_width: u32 = fs::read_to_string(pci_path.join("current_link_width"))
            .unwrap_or_default()
            .trim()
            .parse()
            .unwrap_or(0);

        let max_speed = fs::read_to_string(pci_path.join("max_link_speed"))
            .unwrap_or_else(|_| "Unknown".into())
            .trim()
            .to_string();

        let max_width: u32 = fs::read_to_string(pci_path.join("max_link_width"))
            .unwrap_or_default()
            .trim()
            .parse()
            .unwrap_or(0);

        // A PCIe link is degraded if negotiated width is less than max width (e.g. x4 degraded to x2 or x1)
        let link_degraded = max_width > 0 && current_width > 0 && current_width < max_width;

        PcieLinkState {
            pci_address: pci_addr,
            current_speed,
            current_width,
            max_speed,
            max_width,
            link_degraded,
        }
    }
}
