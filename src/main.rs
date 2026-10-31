//! Main Entrypoint & Real-Time Terminal Telemetry Dashboard
//!
//! Spawns PCIe link monitor, NVMe IOCTL poller, and Prometheus server on :9108.

mod driver;
mod exporter;
mod nvme_spec;
mod pcie_scanner;

use driver::NvmeDriver;
use exporter::{PrometheusExporter, SentinelSnapshot};
use nvme_spec::NvmeSmartMetrics;
use pcie_scanner::{PcieLinkState, PcieScanner};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;

static RUNNING: AtomicBool = AtomicBool::new(true);

fn main() {
    println!("\x1b[2J\x1b[H"); // Clear screen
    println!("\x1b[1;36m========================================================================\x1b[0m");
    println!("\x1b[1;37m        NVME-SENTINEL-RS: Hardware Telemetry & PCIe Link Daemon        \x1b[0m");
    println!("\x1b[1;36m========================================================================\x1b[0m");

    // Discover NVMe controller on host
    let controllers = PcieScanner::discover_controllers();
    let controller = if let Some(ctrl) = controllers.first() {
        ctrl.clone()
    } else {
        println!("\x1b[33m[!] No physical NVMe controller found in /sys/class/nvme. Falling back to synthetic node.\x1b[0m");
        pcie_scanner::ControllerIdentity {
            name: "nvme0".into(),
            model: "Synthetic Enterprise NVMe".into(),
            firmware: "FW_MOCK".into(),
            pci_path: std::path::PathBuf::from("/sys/bus/pci/devices/0000:01:00.0"),
        }
    };

    let pcie_init = PcieScanner::read_pcie_link(&controller.pci_path);
    let dev_char_path = format!("/dev/{}", controller.name);

    let (initial_smart, is_synthetic) = match NvmeDriver::read_smart_log(&dev_char_path) {
        Ok(metrics) => (metrics, false),
        Err(_) => {
            // Unprivileged or sandbox mode -> fallback to realistic metrics
            (NvmeDriver::synthesize_metrics(0), true)
        }
    };

    let state = Arc::new(RwLock::new(SentinelSnapshot {
        controller_name: controller.name.clone(),
        model: controller.model.clone(),
        firmware: controller.firmware.clone(),
        pcie: pcie_init,
        smart: initial_smart,
        is_synthetic,
    }));

    // Start Prometheus Exporter on Port 9108
    let port = 9108;
    PrometheusExporter::start_exporter(Arc::clone(&state), port);
    println!("\x1b[32m[+] Prometheus Metrics Exporter listening on http://0.0.0.0:{}/metrics\x1b[0m", port);

    // Trap SIGINT / SIGTERM for clean shutdown
    ctrlc::set_handler(move || {
        RUNNING.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");

    let mut step: u64 = 0;
    while RUNNING.load(Ordering::SeqCst) {
        step += 1;

        // Poll PCIe link state
        let pcie = PcieScanner::read_pcie_link(&controller.pci_path);

        // Poll NVMe SMART Log
        let smart = match NvmeDriver::read_smart_log(&dev_char_path) {
            Ok(metrics) => metrics,
            Err(_) => NvmeDriver::synthesize_metrics(step),
        };

        // Update shared atomic state
        {
            let mut writer = state.write().unwrap();
            writer.pcie = pcie.clone();
            writer.smart = smart.clone();
        }

        // Render Telemetry Dashboard
        render_dashboard(&controller.name, &controller.model, &controller.firmware, &pcie, &smart, is_synthetic);

        thread::sleep(Duration::from_millis(1000));
    }

    println!("\n\x1b[1;33m[!] Shutdown signal caught. Halting NVMe Sentinel Daemon gracefully.\x1b[0m");
}

fn render_dashboard(
    name: &str,
    model: &str,
    firmware: &str,
    pcie: &PcieLinkState,
    smart: &NvmeSmartMetrics,
    is_synthetic: bool,
) {
    print!("\x1b[H"); // Cursor to home
    println!("\x1b[1;36m========================================================================\x1b[0m");
    println!(
        "\x1b[1;37m NVME HARDWARE TELEMETRY & LINK STATUS  \x1b[32m[ACTIVE]\x1b[0m | Node: \x1b[1;33m{}\x1b[0m",
        name
    );
    println!("\x1b[1;36m========================================================================\x1b[0m");

    println!(" Controller Model:  \x1b[1m{}\x1b[0m", model);
    println!(" Firmware Revision: \x1b[1m{}\x1b[0m", firmware);
    println!(
        " Driver Mode:       {}",
        if is_synthetic {
            "\x1b[33mSynthetic Fallback (Non-root / Sandbox Mode)\x1b[0m"
        } else {
            "\x1b[32mDirect Kernel Admin IOCTL (/dev/nvme0)\x1b[0m"
        }
    );

    println!("\x1b[1;34m---------------------------- PCIe Bus Physical Link --------------------\x1b[0m");
    println!(" Physical PCI Address: \x1b[1m{}\x1b[0m", pcie.pci_address);
    println!(" Negotiated Link Speed: \x1b[1;32m{}\x1b[0m (Max: {})", pcie.current_speed, pcie.max_speed);
    println!(
        " Negotiated Bus Width: \x1b[1;32mx{}\x1b[0m (Max: x{})",
        pcie.current_width, pcie.max_width
    );
    println!(
        " Link Integrity State: {}",
        if pcie.link_degraded {
            "\x1b[1;31m[CRITICAL DEGRADED: PCIe Width Dropped]\x1b[0m"
        } else {
            "\x1b[1;32m[OPTIMAL: Full Lane Width Active]\x1b[0m"
        }
    );

    println!("\x1b[1;34m---------------------------- Flash Health & Wear Diagnostics -----------\x1b[0m");
    let temp_color = if smart.temp_celsius > 70.0 {
        "\x1b[1;31m"
    } else if smart.temp_celsius > 55.0 {
        "\x1b[1;33m"
    } else {
        "\x1b[1;32m"
    };
    println!(" Composite Temp:        {}{:.1}°C\x1b[0m", temp_color, smart.temp_celsius);

    let spare_bar = progress_bar(smart.available_spare_pct as usize, 100);
    println!(" Available Spare:       {}% [{}]", smart.available_spare_pct, spare_bar);

    let wear_bar = progress_bar(smart.percentage_used as usize, 100);
    println!(" Flash Lifespan Used:   {}% [{}]", smart.percentage_used, wear_bar);

    println!(
        " Lifetime Data Read:    {:.2} GB ({} Host Reads)",
        smart.data_read_bytes as f64 / 1_000_000_000.0,
        smart.host_reads_count
    );
    println!(
        " Lifetime Data Written: {:.2} GB ({} Host Writes)",
        smart.data_written_bytes as f64 / 1_000_000_000.0,
        smart.host_writes_count
    );
    println!(" Power On Hours:        {} hrs ({} Power Cycles)", smart.power_on_hours, smart.power_cycles);
    println!(
        " Hardware Faults:       Media Errors: \x1b[1;{}m{}\x1b[0m | Unsafe Shutdowns: \x1b[1;{}m{}\x1b[0m",
        if smart.media_errors > 0 { "31" } else { "32" },
        smart.media_errors,
        if smart.unsafe_shutdowns > 5 { "33" } else { "32" },
        smart.unsafe_shutdowns
    );

    println!("\x1b[1;36m========================================================================\x1b[0m");
    println!(" Prometheus Exporter: http://0.0.0.0:9108/metrics | Press Ctrl+C to stop");
}

fn progress_bar(val: usize, max: usize) -> String {
    let width = 20;
    let filled = (val * width) / max.max(1);
    let empty = width.saturating_sub(filled);
    format!("{}{}", "■".repeat(filled.min(width)), "·".repeat(empty))
}
