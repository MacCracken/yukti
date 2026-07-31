# Yukti

> **Yukti** (Sanskrit: युक्ति — reasoning, contrivance, application) — device abstraction layer for AGNOS

[![License: GPL-3.0](https://img.shields.io/badge/license-GPL--3.0-blue.svg)](LICENSE)

Unified API for detecting, monitoring, and managing hardware devices on Linux — USB storage, optical drives, block devices, GPU, audio, network shares, and udev hotplug events.

**424 KB static binary. Zero dependencies. Direct syscalls.**

Written in [Cyrius](https://github.com/MacCracken/cyrius) — ported from Rust (April 2026).

## Features

| Module | Description |
|--------|-------------|
| **core** | Kernel-safe types: `DeviceInfo` layout, `DeviceClass` (10 types), `DeviceCapabilities` (O(1) bitflags), `DeviceHealth`. No alloc, no syscalls |
| **pci** | Kernel-safe PCI class / vendor tables + predicates. Consumed by the AGNOS kernel |
| **device** | Userland constructors, serializers, sysfs queries over the `core` types |
| **error** | 16 error kinds, heap-allocated error structs, errno mapping |
| **syscalls** | Arch-conditional `SYS_*` constants + the `_yk_mount` / `_yk_umount2` / `_yk_mkdir` agnos ABI bridges |
| **event** | `DeviceEvent` pub/sub with function pointer listeners and class-based filtering |
| **storage** | `mount()` / `unmount()` / `eject()`, filesystem detection (17 types), `/proc/mounts` parsing |
| **optical** | Tray control, disc TOC reading, DVD Video detection, drive status via ioctl |
| **udev** | Netlink hotplug monitor, sysfs enumeration, device classification, uevent parsing |
| **linux** | `LinuxDeviceManager` — ties it all together with hashmap cache |
| **udev_rules** | Rule rendering, validation, udevadm integration |
| **partition** | MBR + GPT table reading, EFI System Partition detection, boot flags |
| **device_db** | Persistent device history, mount preferences, mount/unmount log via patra |
| **network** | SMB/CIFS and NFS mount helpers, share detection via `/proc/mounts` |
| **gpu** | GPU probe via `/sys/class/drm/` — vendor/device IDs, driver, render nodes |
| **audio** | ALSA PCM enumeration (`/dev/snd/`, `/proc/asound/`), vani descriptor adapter |

## Quick Start

```cyrius
include "lib/yukti.cyr"

fn main() {
    alloc_init();

    # Enumerate all block devices
    var mgr = linux_dm_new();
    var r = linux_dm_enumerate(mgr);
    if (is_ok(r)) {
        var devices = payload(r);
        var n = vec_len(devices);
        for (var i = 0; i < n; i = i + 1) {
            var info = vec_get(devices, i);
            var name = device_info_display_name(info);
            str_println(name);
        }
    }
    syscall(60, 0);
}
main();
```

## Build

Requires the [Cyrius toolchain](https://github.com/MacCracken/cyrius) 6.5.3 or newer.

```sh
# Resolve deps into lib/
cyrius deps

# Build the CLI
cyrius build src/main.cyr build/yukti

# Run
./build/yukti

# Test (658 assertions)
cyrius test tests/tcyr/yukti.tcyr

# Benchmark
cyrius bench tests/bcyr/yukti.bcyr

# Fuzz
cyrius build fuzz/fuzz_parse_uevent.fcyr    build/fuzz_parse_uevent    && ./build/fuzz_parse_uevent
cyrius build fuzz/fuzz_mount_table.fcyr     build/fuzz_mount_table     && ./build/fuzz_mount_table
cyrius build fuzz/fuzz_partition_table.fcyr build/fuzz_partition_table && ./build/fuzz_partition_table

# Bundle for distribution
cyrius distlib               # dist/yukti.cyr       (full userland)
cyrius distlib core          # dist/yukti-core.cyr  (kernel-safe: core + pci)

# Supply-chain integrity
cyrius deps --lock           # cyrius.lock
cyrius deps --verify
```

## Example Output

```
yukti device enumeration
========================

 Found 3 device(s)

  [1] /dev/nvme0n1  block-internal  ready  1.8 TB  CT2000P3SSD8
  [2] /dev/sda  block-internal  ready  1.8 TB  WD Blue SA510 2.
  [3] /dev/zram0  block-internal  ready  29.8 GB  /dev/zram0
```

## API Overview

### Device Detection
```cyrius
var mgr = linux_dm_new();
var r = linux_dm_enumerate(mgr);
var devices = payload(r);
```

### Mount/Unmount
```cyrius
var opts = mount_options_new();
var r = linux_dm_mount(mgr, device_id, opts);
linux_dm_unmount(mgr, device_id);
linux_dm_eject(mgr, device_id);
```

### Hotplug Monitoring
```cyrius
var mon_r = udev_monitor_new();
var mon = payload(mon_r);
udev_monitor_run(mon, &my_event_handler);
```

### Optical Drives
```cyrius
open_tray("/dev/sr0");
close_tray("/dev/sr0");
var status = drive_status("/dev/sr0");
var toc = read_toc("/dev/sr0");
```

### Filesystem Detection
```cyrius
var fs = filesystem_from_str("ext4");          # FS_EXT4
var writable = filesystem_is_writable(fs);     # 1
var optical = filesystem_is_optical(fs);       # 0
```

## Consumers

- **jalwa** — auto-import music from USB/CD (hotplug -> detect -> mount -> import)
- **file manager** — device sidebar with mount/eject actions
- **aethersafha** — desktop mount/unmount notifications
- **argonaut** — policy-driven automount on boot
- **AGNOS kernel** — kernel-safe `dist/yukti-core.cyr` subset (`core` + `pci`: PCI class/vendor tables, zero alloc, zero syscalls)

## Architecture

```
┌────────────────────────────────────────────────────────┐
│                   LinuxDeviceManager                   │
│  enumerate() / get() / refresh() / mount() / eject()   │
├──────────────┬───────────────┬─────────────────────────┤
│    udev      │   storage     │      optical            │
│  netlink     │  mount/eject  │   tray/TOC/ioctl        │
│  sysfs enum  │  /proc/mounts │   disc detection        │
├──────────────┼───────────────┼─────────────────────────┤
│  partition   │   network     │        gpu              │
│  MBR / GPT   │  SMB / NFS    │   sysfs DRM probe       │
├──────────────┼───────────────┼─────────────────────────┤
│    audio     │  device_db    │     udev_rules          │
│  ALSA PCM    │  patra hist   │   render / validate     │
├──────────────┴───────────────┴─────────────────────────┤
│          device / event / error / core / pci           │
│  DeviceInfo, DeviceClass, Capabilities, EventListener  │
├────────────────────────────────────────────────────────┤
│         Linux syscalls (direct, src/syscalls.cyr)      │
│    mount / umount2 / ioctl / socket / ppoll via SYS_*  │
└────────────────────────────────────────────────────────┘
```

## Rust vs Cyrius

Frozen port-day snapshot — the final Rust tree measured against yukti 1.0.0, the
Cyrius port, on the day the Rust implementation was deleted (April 2026). These
are historical numbers, **not** current state; see [Features](#features) above
for what yukti looks like today. Full breakdown in
[docs/benchmarks/rust-v-cyrius.md](docs/benchmarks/rust-v-cyrius.md).

| Metric (April 2026) | Rust (final) | Cyrius (port day) |
|---------------------|--------------|-------------------|
| Binary size | 449 KB | 152 KB |
| Dependencies | 47 crates | 0 |
| Source lines | 6,166 | 3,359 |
| Tests | 229 | 407 |
| Benchmarks | 48 | 45 |

## License

GPL-3.0-only
