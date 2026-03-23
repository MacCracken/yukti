# Yukti

> **Yukti** (Sanskrit: यन्त्र — device, instrument, machine) — device abstraction layer for AGNOS

[![CI](https://github.com/MacCracken/yukti/actions/workflows/ci.yml/badge.svg)](https://github.com/MacCracken/yukti/actions/workflows/ci.yml)
[![License: GPL-3.0](https://img.shields.io/badge/license-GPL--3.0-blue.svg)](LICENSE)
[![crates.io](https://img.shields.io/crates/v/yukti.svg)](https://crates.io/crates/yukti)
[![docs.rs](https://docs.rs/yukti/badge.svg)](https://docs.rs/yukti)

Unified API for detecting, monitoring, and managing hardware devices on Linux — USB storage, optical drives, block devices, and udev hotplug events.

**Linux-native, zero-alloc hot paths** — built on `libc` syscalls, bitflag capabilities, `tracing` instrumentation.

## Features

| Module | Feature | Description |
|--------|---------|-------------|
| **device** | always | `DeviceInfo`, `DeviceClass` (8 types), `DeviceCapabilities` (O(1) bitflags), `DeviceHealth` |
| **event** | always | `DeviceEvent` pub/sub with `EventListener` trait and class-based filtering |
| **storage** | `storage` | `mount()` / `unmount()` / `eject()`, filesystem detection (17 types), `/proc/mounts` parsing |
| **optical** | `optical` | Tray control, disc TOC reading, DVD Video detection, drive status via ioctl |
| **udev** | `udev` | Netlink hotplug monitor, sysfs enumeration, device classification, uevent parsing |
| **linux** | always | `LinuxDeviceManager` — ties it all together with `Arc` cache and lifecycle management |

Default features: `udev`, `storage`, `optical`.

Optional: `ai` (daimon/hoosh integration, requires `reqwest`, `tokio`).

```toml
[dependencies]
yukti = "0.22"
```

## Quick Start

### Enumerate Devices

```rust
use yukti::{LinuxDeviceManager, Device};

let mgr = LinuxDeviceManager::new();
for dev in mgr.enumerate().unwrap() {
    println!("{}: {} [{}] {}", dev.id, dev.display_name(), dev.class, dev.size_display());
}
```

### Monitor Hotplug Events

```rust
use yukti::{LinuxDeviceManager, Device};

let mgr = LinuxDeviceManager::new();
let rx = mgr.start_monitor().unwrap();

for event in rx.iter() {
    println!("{}: {} on {}", event.device_id, event.kind, event.dev_path.display());
}
```

### Mount a Device

```rust
use yukti::storage::{MountOptions, mount};
use std::path::Path;

let result = mount(
    Path::new("/dev/sdb1"),
    &MountOptions::new()
        .mount_point("/mnt/usb")
        .read_only(true),
).unwrap();

println!("Mounted {} at {}", result.fs_type, result.mount_point.display());
```

### Optical Drive

```rust
use yukti::optical::{open_tray, read_toc, is_dvd_video};
use std::path::Path;

let dev = Path::new("/dev/sr0");
let toc = read_toc(dev).unwrap();
println!("{}: {} tracks", toc.disc_type, toc.tracks.len());

open_tray(dev).unwrap();
```

### Filesystem & Device Queries

```rust
use yukti::storage::{Filesystem, filesystem_usage};
use yukti::device::{query_permissions, query_device_health};
use std::path::Path;

let fs: Filesystem = "ext4".into();
assert!(fs.is_writable());

let usage = filesystem_usage(Path::new("/")).unwrap();
println!("{:.1}% used", usage.usage_percent);

let health = query_device_health("nvme0n1");
if let Some(temp) = health.temperature_celsius {
    println!("NVMe temp: {temp}°C");
}
```

## Modules

| Module | Key Types |
|--------|-----------|
| `device` | `DeviceInfo`, `DeviceId`, `DeviceClass`, `DeviceCapabilities`, `DeviceState`, `DevicePermissions`, `DeviceHealth`, `Device` trait |
| `event` | `DeviceEvent`, `DeviceEventKind`, `EventListener` trait, `EventCollector` |
| `storage` | `Filesystem` (17 types), `MountOptions` (builder), `MountResult`, `FilesystemUsage`, `mount()`, `unmount()`, `eject()` |
| `optical` | `DiscType` (10), `TrayState`, `DiscToc`, `open_tray()`, `close_tray()`, `drive_status()`, `read_toc()`, `is_dvd_video()` |
| `udev` | `UdevEvent`, `UdevMonitor` (netlink, filtered, bounded), `classify_device()`, `enumerate_devices()` |
| `linux` | `LinuxDeviceManager` — `Device` impl, `mount()`, `unmount()`, `eject()`, `start_monitor()`, `dispatch_event()` |
| `error` | `YuktiError` (15 variants) |

## Feature Flags

| Feature | Default | Description |
|---------|---------|-------------|
| `udev` | Yes | Device enumeration and hotplug via sysfs + netlink |
| `storage` | Yes | Mount/unmount/eject via `libc` syscalls |
| `optical` | Yes | Tray control and TOC reading via CD-ROM ioctls |
| `ai` | No | AI-assisted device management (planned) |

## Consumers

| Project | Integration |
|---------|------------|
| **jalwa** | Hotplug → detect → mount → auto-import music from USB/CD |
| **file manager** | Device sidebar, mount/eject actions |
| **aethersafha** | Desktop mount/unmount notifications |
| **argonaut** | Policy-driven automount on boot |

## License

GPL-3.0-only — see [LICENSE](LICENSE) for details.
