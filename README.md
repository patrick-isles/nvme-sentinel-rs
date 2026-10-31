# NVMe-Sentinel-RS (`nvme-sentinel-rs`)

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust: 2021 Edition](https://img.shields.io/badge/Rust-2021%20Edition-orange.svg)](https://www.rust-lang.org/)
[![Subsystem: NVMe 2.0 / 1.4](https://img.shields.io/badge/Spec-NVMe%20Base%202.0-red.svg)](https://nvmexpress.org/)
[![Bus: PCIe Gen 4/5](https://img.shields.io/badge/Bus-PCIe%2016.0%20GT%2Fs%20x4-brightgreen.svg)](docs/SYSTEMS_ARCHITECTURE.md)
[![Telemetry: Prometheus](https://img.shields.io/badge/Metrics-Prometheus%20:9108-orange.svg)](http://localhost:9108/metrics)

> **Production-grade NVMe & PCIe Physical Link Telemetry, SMART Health Analytics, Hardware Fault Sentinel & Prometheus Exporter in pure Rust.**

`nvme-sentinel-rs` monitors flash memory wear, controller thermal transitions, and PCIe bus physical link integrity on bare-metal enterprise Linux servers, high-density storage nodes, and rugged industrial computing units. It directly issues raw NVMe Admin IOCTL commands (`NVME_IOCTL_ADMIN_CMD`), unpacks the 512-byte binary SMART log page with zero heap allocations, interrogates Linux `/sys/bus/pci` for link-speed degradation (e.g. x4 to x2/x1 drops), and exports real-time observability metrics to **Prometheus on port `:9108`** alongside a live terminal dashboard.

---

## Architecture Topology

```
             ┌─────────────────────────────────────────────────────────┐
             │         PHYSICAL HARDWARE: SOLID-STATE STORAGE          │
             │   • NVMe Controller (/dev/nvme0)                        │
             │   • PCIe Gen 4/5 Root Complex (0000:06:00.0)            │
             └──────────────────────────┬──────────────────────────────┘
                                        │
           ┌────────────────────────────┴────────────────────────────┐
           │                                                         │
           ▼                                                         ▼
┌───────────────────────────────────────┐ ┌───────────────────────────────────────┐
│     NVME DRIVER IOCTL WORKER          │ │     PCIE SYSFS LINK SCANNER           │
│  • Binds to `/dev/nvme0` (Raw I/O)    │ │  • Traverses `/sys/bus/pci/devices/`  │
│  • Opcode 0x02: NVME_ADMIN_GET_LOG    │ │  • Tracks `current_link_speed`        │
│  • LID 0x02: SMART / Health (512B)    │ │  • Tracks `current_link_width` (x4)   │
│  • Zero-copy `repr(C, packed)` unpack │ │  • Detects silent lane degradation    │
└──────────────────┬────────────────────┘ └──────────────────┬────────────────────┘
                   │                                         │
                   └────────────────────┬────────────────────┘
                                        │
                                        ▼ (Thread-Safe Arc<RwLock<SentinelSnapshot>>)
┌─────────────────────────────────────────────────────────────────────────────────┐
│                      CORE SENTINEL ANALYTICS ENGINE                             │
│                                                                                 │
│  • Composite Temperature Gradient Monitoring (Kelvin -> Celsius)                │
│  • Flash Endurance Consumption: (Percentage Used, Available Spare Ratio)        │
│  • Unrecoverable Media Integrity Fault Counters & Unsafe Shutdown Audits        │
│  • Bus Link State Verification (Optimal x4 vs. Degraded Alert)                  │
└───────────────────────────────────────┬─────────────────────────────────────────┘
                                        │
             ┌──────────────────────────┴──────────────────────────┐
             ▼                                                     ▼
┌───────────────────────────────────────┐ ┌───────────────────────────────────────┐
│    PROMETHEUS HTTP EXPORTER (:9108)   │ │      LIVE TERMINAL DASHBOARD          │
│  • `nvme_composite_temperature_celsius`│ │  • Model: Micron 2450 / Samsung PM9A1│
│  • `nvme_available_spare_ratio`       │ │  • High-refresh ASCII health gauges   │
│  • `nvme_percentage_used_ratio`       │ │  • Real-time I/O throughput counters  │
│  • `pcie_current_link_width_lanes`    │ │  • Hardware fault alerts              │
│  • `pcie_link_degraded_status` (0/1)  │ │  • Zero external runtime dependencies │
└───────────────────────────────────────┘ └───────────────────────────────────────┘
```

---

## Live Terminal Dashboard Preview

```text
========================================================================
 NVME HARDWARE TELEMETRY & LINK STATUS  [ACTIVE] | Node: nvme0
========================================================================
 Controller Model:  Micron_2450_MTFDKBA1T0TFK
 Firmware Revision: V5MA010
 Driver Mode:       Direct Kernel Admin IOCTL (/dev/nvme0)
---------------------------- PCIe Bus Physical Link --------------------
 Physical PCI Address: 0000:06:00.0
 Negotiated Link Speed: 16.0 GT/s PCIe (Max: 16.0 GT/s PCIe)
 Negotiated Bus Width: x4 (Max: x4)
 Link Integrity State: [OPTIMAL: Full Lane Width Active]
---------------------------- Flash Health & Wear Diagnostics -----------
 Composite Temp:        44.0°C
 Available Spare:       100% [■■■■■■■■■■■■■■■■■■■■]
 Flash Lifespan Used:   3% [····················]
 Lifetime Data Read:    4520.00 GB (85420002 Host Reads)
 Lifetime Data Written: 3150.00 GB (62110002 Host Writes)
 Power On Hours:        1280 hrs (412 Power Cycles)
 Hardware Faults:       Media Errors: 0 | Unsafe Shutdowns: 2
========================================================================
 Prometheus Exporter: http://0.0.0.0:9108/metrics | Press Ctrl+C to stop
```

---

## Quickstart

### Prerequisites
- Linux Kernel 5.4+ with an NVMe solid-state storage controller.
- Rust toolchain (2021 Edition).

### Build & Run
```bash
# Clone the repository
git clone https://github.com/patrick-isles/nvme-sentinel-rs.git
cd nvme-sentinel-rs

# Compile optimized release binary
cargo build --release

# Run with elevated privileges for raw character device IOCTL access
sudo ./target/release/nvme-sentinel-rs
```

*(Note: If executed in an unprivileged or containerized environment, `nvme-sentinel-rs` automatically engages its synthetic fallback engine to allow full Prometheus and dashboard validation without crashing).*

---

## Prometheus Metrics Reference

| Metric Identifier | Metric Type | Description |
|---|---|---|
| `nvme_composite_temperature_celsius` | Gauge | Current temperature of the controller in Celsius. |
| `nvme_available_spare_ratio` | Gauge | Remaining spare flash block capacity normalized from `0.0` to `1.0`. |
| `nvme_percentage_used_ratio` | Gauge | Estimated device lifespan consumed (`0.0` = fresh, `1.0` = 100% TBW reached). |
| `nvme_data_read_bytes_total` | Counter | Cumulative bytes read by host applications from flash memory. |
| `nvme_data_written_bytes_total` | Counter | Cumulative bytes written to the solid-state medium. |
| `nvme_media_errors_total` | Counter | Total unrecoverable data integrity or uncorrectable ECC errors. |
| `pcie_current_link_width_lanes` | Gauge | Currently negotiated PCIe physical link width in lanes (e.g. `4`). |
| `pcie_link_degraded_status` | Gauge | Alert gauge: `1` if PCIe link has degraded below max capable width, `0` if optimal. |

---

## Systemd Hardened Service

Deploy as an enterprise background daemon with Linux namespace isolation and strict capability bounding:

```bash
sudo cp target/release/nvme-sentinel-rs /usr/local/bin/
sudo cp systemd/nvme-sentinel.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now nvme-sentinel
```

---

## Author & License

- **Author**: Patrick Isles `<patrick.isles@pcc.edu>`
- **Repository**: [https://github.com/patrick-isles/nvme-sentinel-rs](https://github.com/patrick-isles/nvme-sentinel-rs)
- **License**: MIT
