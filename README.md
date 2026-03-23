# Yantra

> **Yantra** (Sanskrit: यन्त्र — device, instrument, machine) — device abstraction layer for AGNOS

[![License: GPL-3.0](https://img.shields.io/badge/license-GPL--3.0-blue.svg)](LICENSE)

Yantra provides a unified API for detecting, monitoring, and managing hardware devices on Linux: USB storage, optical drives, block devices, and udev hotplug events.

## Architecture

```
yantra (this crate)
  ├── libc (Linux syscalls: mount, ioctl, netlink)
  ├── bitflags (O(1) capability checks)
  └── tracing (structured logging)

Consumers:
  jalwa       ──→ yantra (auto-import music from USB/CD)
  file-mgr    ──→ yantra (device sidebar, mount/eject actions)
  aethersafha ──→ yantra (desktop mount notifications)
  argonaut    ──→ yantra (automount policy on boot)
```

## Features

- **Device detection** — enumerate block devices from sysfs, classify by type (USB, optical, SD, NVMe, loop, device-mapper)
- **Capability bitflags** — O(1) membership checks via `DeviceCapabilities` (read, write, eject, TRIM, tray control, burn, etc.)
- **Mount/unmount/eject** — `libc::mount()` with auto-detect filesystem, `umount2()`, optical tray eject via ioctl, USB safe-remove via sysfs
- **Optical drive control** — open/close tray, drive status, read disc TOC (track listing, durations, disc type detection)
- **Udev hotplug monitor** — netlink socket listener for real-time device attach/detach/change events with channel-based subscription
- **Event system** — `DeviceEvent` with `EventListener` trait, class-based filtering, thread-safe `EventCollector`
- **Device manager** — `LinuxDeviceManager` implements `Device` trait, ties together enumeration, monitoring, mount/eject lifecycle
- **Filesystem detection** — 12 known types with zero-allocation case-insensitive parsing
- **Structured logging** — `tracing` instrumentation on all I/O operations (mount, unmount, eject, tray, monitor, enumerate)
- **Serde support** — all types serialize/deserialize (JSON-compatible capability bitflags)

## Quick Start

```rust
use yantra::{LinuxDeviceManager, Device, DeviceClass};
use yantra::storage::Filesystem;
use yantra::optical::{DiscType, detect_disc_type};

// Enumerate all connected devices
let mgr = LinuxDeviceManager::new();
let devices = mgr.enumerate().unwrap();
for dev in &devices {
    println!("{}: {} ({})", dev.id, dev.display_name(), dev.class);
}

// Parse filesystem type
let fs = Filesystem::from_str_type("ext4");
assert!(fs.is_writable());

// Detect disc type
let disc = detect_disc_type("cd", true, false);
assert_eq!(disc, DiscType::CdAudio);

// Monitor hotplug events
let rx = mgr.start_monitor().unwrap();
for event in rx.iter() {
    println!("{}: {} on {}", event.device_id, event.kind, event.dev_path.display());
}
```

## Modules

| Module | Description |
|--------|-------------|
| `device` | `DeviceInfo`, `DeviceId`, `DeviceClass` (8 types), `DeviceCapabilities` (bitflags), `DeviceState`, `Device` trait |
| `event` | `DeviceEvent`, `DeviceEventKind`, `EventListener` trait, `EventCollector` |
| `storage` | `Filesystem` (12 types), `MountOptions`, `mount()`, `unmount()`, `eject()`, mount point validation |
| `optical` | `DiscType` (10 types), `TrayState`, `DiscToc`, `open_tray()`, `close_tray()`, `drive_status()`, `read_toc()` |
| `udev` | `UdevEvent`, `UdevMonitor` (netlink), `classify_device()`, `enumerate_devices()`, uevent parsing |
| `linux` | `LinuxDeviceManager` — `Device` trait impl, mount/eject lifecycle, hotplug monitor management |
| `error` | `YantraError` with 11 variants |

## Feature Flags

| Feature | Default | Description |
|---------|---------|-------------|
| `udev` | Yes | udev device enumeration and hotplug (requires `libc`) |
| `storage` | Yes | Mount/unmount/eject operations (requires `libc`) |
| `optical` | Yes | Optical drive support — tray control, TOC reading (requires `libc`) |
| `ai` | No | Daimon/hoosh AI integration (requires `reqwest`, `tokio`) |

## Consumers

- **jalwa** — auto-import music from USB/CD
- **file manager** — device sidebar, mount/eject actions
- **aethersafha** — desktop mount notifications
- **argonaut** — automount policy on boot

## License

GPL-3.0-only — see [LICENSE](LICENSE) for details.
