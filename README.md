# Yantra

> **Yantra** (Sanskrit: यन्त्र — device, instrument, machine) — device abstraction layer for AGNOS

Hardware device detection, monitoring, and management. Provides a unified API for USB storage, optical drives, block devices, and udev hotplug events.

## Architecture

```
yantra/src/
├── lib.rs      — module declarations, re-exports
├── device.rs   — DeviceInfo, DeviceClass, DeviceCapability, DeviceState, Device trait
├── event.rs    — DeviceEvent, DeviceEventKind, EventListener trait
├── storage.rs  — Filesystem detection, mount/unmount, eject, mount point management
├── optical.rs  — DiscType, TrayState, TOC reading, disc detection
├── udev.rs     — udev event parsing, device classification, capability extraction
└── error.rs    — YantraError
```

## Usage

```rust
use yantra::{DeviceInfo, DeviceClass, DeviceId};
use yantra::storage::Filesystem;
use yantra::optical::{DiscType, detect_disc_type};
use yantra::udev::{classify_device, device_info_from_udev};

// Detect disc type
let disc = detect_disc_type("cd", true, false);
assert_eq!(disc, DiscType::CdAudio);

// Parse filesystem
let fs = Filesystem::from_str_type("ext4");
assert!(fs.is_writable());
```

## Feature Flags

| Feature | Default | Description |
|---------|---------|-------------|
| `udev` | Yes | udev device enumeration and hotplug |
| `storage` | Yes | Mount/unmount/eject operations |
| `optical` | Yes | Optical drive support |
| `ai` | No | Daimon/hoosh AI integration |

## Consumers

- **jalwa** — auto-import music from USB/CD
- **file manager** — device sidebar, mount/eject actions
- **aethersafha** — desktop mount notifications
- **argonaut** — automount policy on boot

## License

GPL-3.0
