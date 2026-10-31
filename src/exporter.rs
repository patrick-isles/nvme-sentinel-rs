//! Prometheus HTTP Metrics Exporter (:9108/metrics)
//!
//! Exposes production-ready OpenMetrics / Prometheus gauge and counter timeseries.

use crate::nvme_spec::NvmeSmartMetrics;
use crate::pcie_scanner::PcieLinkState;
use std::sync::{Arc, RwLock};
use std::thread;
use tiny_http::{Response, Server};

#[derive(Debug, Clone)]
pub struct SentinelSnapshot {
    pub controller_name: String,
    pub model: String,
    pub firmware: String,
    pub pcie: PcieLinkState,
    pub smart: NvmeSmartMetrics,
    pub is_synthetic: bool,
}

pub struct PrometheusExporter;

impl PrometheusExporter {
    pub fn start_exporter(state: Arc<RwLock<SentinelSnapshot>>, port: u16) {
        thread::spawn(move || {
            let addr = format!("0.0.0.0:{}", port);
            let server = match Server::http(&addr) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Prometheus HTTP server failed to bind to {}: {}", addr, e);
                    return;
                }
            };

            for request in server.incoming_requests() {
                if request.url() == "/metrics" || request.url() == "/" {
                    let snapshot = state.read().unwrap().clone();

                    let mut body = String::with_capacity(2048);
                    body.push_str("# HELP nvme_composite_temperature_celsius Current temperature of the NVMe controller in Celsius\n");
                    body.push_str("# TYPE nvme_composite_temperature_celsius gauge\n");
                    body.push_str(&format!(
                        "nvme_composite_temperature_celsius{{controller=\"{}\",model=\"{}\"}} {:.2}\n",
                        snapshot.controller_name, snapshot.model, snapshot.smart.temp_celsius
                    ));

                    body.push_str("# HELP nvme_available_spare_ratio Remaining spare flash blocks normalized to 0.0-1.0\n");
                    body.push_str("# TYPE nvme_available_spare_ratio gauge\n");
                    body.push_str(&format!(
                        "nvme_available_spare_ratio{{controller=\"{}\"}} {:.2}\n",
                        snapshot.controller_name,
                        snapshot.smart.available_spare_pct as f64 / 100.0
                    ));

                    body.push_str("# HELP nvme_percentage_used_ratio Percentage of device lifespan consumed normalized to 0.0-1.0\n");
                    body.push_str("# TYPE nvme_percentage_used_ratio gauge\n");
                    body.push_str(&format!(
                        "nvme_percentage_used_ratio{{controller=\"{}\"}} {:.2}\n",
                        snapshot.controller_name,
                        snapshot.smart.percentage_used as f64 / 100.0
                    ));

                    body.push_str("# HELP nvme_data_read_bytes_total Total bytes read from flash storage\n");
                    body.push_str("# TYPE nvme_data_read_bytes_total counter\n");
                    body.push_str(&format!(
                        "nvme_data_read_bytes_total{{controller=\"{}\"}} {}\n",
                        snapshot.controller_name, snapshot.smart.data_read_bytes
                    ));

                    body.push_str("# HELP nvme_data_written_bytes_total Total bytes written to flash storage\n");
                    body.push_str("# TYPE nvme_data_written_bytes_total counter\n");
                    body.push_str(&format!(
                        "nvme_data_written_bytes_total{{controller=\"{}\"}} {}\n",
                        snapshot.controller_name, snapshot.smart.data_written_bytes
                    ));

                    body.push_str("# HELP nvme_media_errors_total Count of unrecoverable data integrity or media errors\n");
                    body.push_str("# TYPE nvme_media_errors_total counter\n");
                    body.push_str(&format!(
                        "nvme_media_errors_total{{controller=\"{}\"}} {}\n",
                        snapshot.controller_name, snapshot.smart.media_errors
                    ));

                    body.push_str("# HELP pcie_current_link_width_lanes Currently negotiated PCIe physical link width in lanes\n");
                    body.push_str("# TYPE pcie_current_link_width_lanes gauge\n");
                    body.push_str(&format!(
                        "pcie_current_link_width_lanes{{controller=\"{}\",pci_addr=\"{}\"}} {}\n",
                        snapshot.controller_name, snapshot.pcie.pci_address, snapshot.pcie.current_width
                    ));

                    body.push_str("# HELP pcie_link_degraded_status 1 if physical PCIe link has degraded below max capable width, 0 otherwise\n");
                    body.push_str("# TYPE pcie_link_degraded_status gauge\n");
                    body.push_str(&format!(
                        "pcie_link_degraded_status{{controller=\"{}\"}} {}\n",
                        snapshot.controller_name,
                        if snapshot.pcie.link_degraded { 1 } else { 0 }
                    ));

                    let response = Response::from_string(body)
                        .with_header(tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"text/plain; version=0.0.4"[..]).unwrap());
                    let _ = request.respond(response);
                } else {
                    let response = Response::from_string("Not Found").with_status_code(404);
                    let _ = request.respond(response);
                }
            }
        });
    }
}
