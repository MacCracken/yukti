# Architecture Overview

## Module Map

```
yukti
├── device          — DeviceInfo, DeviceId, DeviceClass, DeviceCapabilities (bitflags), Device trait
├── event           — DeviceEvent, DeviceEventKind, EventListener trait, EventCollector
├── storage         — Filesystem, mount/unmount/eject, /proc/mounts parsing       [feature: storage]
├── optical         — DiscType, TrayState, DiscToc, tray control, TOC reading     [feature: optical]
├── udev            — UdevEvent, UdevMonitor (netlink), device classification     [feature: udev]
├── linux           — LinuxDeviceManager (Device trait impl, lifecycle management)
└── error           — YuktiError (11 variants)
```

## Feature Flags

| Flag | Dependencies | Description |
|------|-------------|-------------|
| `udev` | `libc` | udev device enumeration via sysfs, hotplug monitoring via netlink |
| `storage` | `libc` | Mount/unmount/eject operations via syscalls and ioctls |
| `optical` | `libc` | Optical drive tray control and TOC reading via CD-ROM ioctls |
| `ai` | `reqwest`, `tokio` | AI integration (planned — see [roadmap](../development/roadmap.md)) |

`udev`, `storage`, and `optical` are enabled by default. The `ai` feature is opt-in.

## Design Principles

- **Linux-native** — direct syscalls via `libc` (mount, umount2, ioctl, netlink), no shelling out
- **Feature-gated** — hardware I/O behind feature flags; core types work without `libc`
- **Bitflag capabilities** — `DeviceCapabilities` is a u16 bitfield for O(1) checks, serializes as `Vec<DeviceCapability>` for JSON compatibility
- **Zero-alloc hot paths** — case-insensitive parsing uses `eq_ignore_ascii_case`, `Cow` returns avoid cloning, byte-level path matching avoids `to_string_lossy`
- **Testable I/O** — parsing functions take strings/paths so tests can pass mock data (e.g., `find_mount_in()` takes a mount table string)
- **Thread-safe** — `LinuxDeviceManager` uses `RwLock`/`Mutex`, `EventCollector` is `Send + Sync`
- **Structured logging** — `tracing` instrumentation on all I/O, no `println!`

## Data Flow

### Device Enumeration

```
/sys/block/*
    │
    ▼
read_sysfs_attr() → build synthetic UdevEvent
    │
    ▼
classify_and_extract() → (DeviceClass, DeviceCapabilities)
    │
    ▼
device_info_from_udev() → DeviceInfo
    │
    ▼
LinuxDeviceManager cache (HashMap<DeviceId, DeviceInfo>)
```

### Hotplug Monitoring

```
kernel (netlink KOBJECT_UEVENT)
    │
    ▼
UdevMonitor::poll() → recv() → parse_uevent()
    │
    ▼
udev_event_to_device_event() → DeviceEvent
    │
    ├──→ mpsc::channel (subscribe API)
    └──→ EventListener::on_event() (listener API)
```

### Mount Lifecycle

```
LinuxDeviceManager::mount(id, options)
    │
    ├── validate_mount_point()
    ├── create_dir_all()
    ├── parse_mount_flags()
    │
    ├── [explicit fs_type] → libc::mount()
    │
    └── [auto-detect] → try ext4, vfat, ntfs, iso9660, udf, exfat, btrfs, xfs
                              │
                              ▼
                        MountResult { fs_type, mount_point }
```

### Optical Drive Operations

```
open_optical_device(dev_path)  →  O_RDONLY | O_NONBLOCK
    │
    ├── open_tray()      → ioctl(CDROMEJECT)
    ├── close_tray()     → ioctl(CDROMCLOSETRAY)
    ├── drive_status()   → ioctl(CDROM_DRIVE_STATUS) → TrayState
    └── read_toc()       → ioctl(CDROMREADTOCHDR)
                          → ioctl(CDROMREADTOCENTRY) × N tracks
                          → DiscToc { disc_type, tracks, total_size_bytes }
```

## Device Classification

Devices are classified from udev properties in priority order:

| Priority | Check | Class |
|----------|-------|-------|
| 1 | `ID_BUS=usb` + disk/partition | `UsbStorage` |
| 2 | `ID_CDROM` present | `Optical` |
| 3 | `ID_BUS=mmc` or `ID_DRIVE_FLASH_SD` | `SdCard` |
| 4 | sysfs path contains `/dm-` | `DeviceMapper` |
| 5 | sysfs path contains `/loop` | `Loop` |
| 6 | block subsystem + disk/partition | `BlockInternal` |
| 7 | fallback | `Unknown` |

## Consumers

| Project | Integration |
|---------|------------|
| jalwa | Auto-import music from USB/CD (hotplug → detect → mount → import) |
| file manager | Device sidebar, mount/eject actions |
| aethersafha | Desktop mount/unmount notifications |
| argonaut | Automount policy engine on boot |
