# Yukti

> **Yukti** (Sanskrit: युक्ति — reasoning, contrivance, application) — device abstraction layer for AGNOS

[![License: GPL-3.0](https://img.shields.io/badge/license-GPL--3.0-blue.svg)](LICENSE)

Unified API for detecting, monitoring, and managing hardware devices on Linux — USB storage, optical drives, block devices, and udev hotplug events.

**152 KB static binary. Zero dependencies. Direct syscalls.**

Written in [Cyrius](https://github.com/MacCracken/cyrius) — ported from Rust (April 2026).

## Features

| Module | Description |
|--------|-------------|
| **device** | `DeviceInfo`, `DeviceClass` (8 types), `DeviceCapabilities` (O(1) bitflags), `DeviceHealth` |
| **event** | `DeviceEvent` pub/sub with function pointer listeners and class-based filtering |
| **storage** | `mount()` / `unmount()` / `eject()`, filesystem detection (17 types), `/proc/mounts` parsing |
| **optical** | Tray control, disc TOC reading, DVD Video detection, drive status via ioctl |
| **udev** | Netlink hotplug monitor, sysfs enumeration, device classification, uevent parsing |
| **linux** | `LinuxDeviceManager` — ties it all together with hashmap cache |
| **udev_rules** | Rule rendering, validation, udevadm integration |

## Quick Start

```cyrius
include "yukti/lib.cyr"

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

Requires the [Cyrius toolchain](https://github.com/MacCracken/cyrius) 5.2.x.

```sh
# Resolve deps into lib/
cyrius deps

# Build the CLI
cyrius build src/main.cyr build/yukti

# Run
./build/yukti

# Test (485 assertions)
cyrius test tests/tcyr/yukti.tcyr

# Benchmark
cyrius bench tests/bcyr/yukti.bcyr

# Fuzz
cyrius build fuzz/fuzz_parse_uevent.fcyr build/fuzz_parse_uevent && ./build/fuzz_parse_uevent
cyrius build fuzz/fuzz_mount_table.fcyr  build/fuzz_mount_table  && ./build/fuzz_mount_table

# Bundle for distribution
cyrius distlib               # dist/yukti.cyr

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

## Architecture

```
┌──────────────────────────────────────────────────────┐
│                  LinuxDeviceManager                    │
│  enumerate() / get() / refresh() / mount() / eject()  │
├──────────────┬───────────────┬────────────────────────┤
│    udev      │   storage     │      optical           │
│  netlink     │  mount/eject  │   tray/TOC/ioctl       │
│  sysfs enum  │  /proc/mounts │   disc detection       │
├──────────────┴───────────────┴────────────────────────┤
│              device / event / error                    │
│  DeviceInfo, DeviceClass, Capabilities, EventListener  │
├───────────────────────────────────────────────────────┤
│              Linux syscalls (direct)                   │
│  mount(165), umount2(166), ioctl(16), socket(41)      │
└───────────────────────────────────────────────────────┘
```

## Rust vs Cyrius

See [docs/benchmarks/rust-v-cyrius.md](docs/benchmarks/rust-v-cyrius.md) for the full comparison.

| Metric | Rust | Cyrius |
|--------|------|--------|
| Binary size | 449 KB | 152 KB |
| Dependencies | 47 crates | 0 |
| Source lines | 6,166 | 3,359 |
| Tests | 229 | 407 |
| Benchmarks | 48 | 45 |

## License

GPL-3.0-only
