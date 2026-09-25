# Architecture Overview

## Module Map

```
yukti (Cyrius)
├── syscalls.cyr    — _yk_* bridges over the stdlib syscall wrappers (agnos
│                     ABI splits + fail-closed arms) and the one raw call, ppoll
├── error.cyr       — 16 error kinds, heap-allocated error structs, errno mapping
├── core.cyr        — Kernel-safe types: DeviceClass (10), DeviceState (6),
│                     DeviceCapabilities (bitflags), DeviceInfo (168 bytes,
│                     21 fields) and DeviceHealth layouts, accessors, predicates
├── pci.cyr         — Kernel-safe PCI class/subclass + vendor/device lookup
│                     tables, pure predicates (storage, nvme, sata, gpu, network)
├── device.cyr      — Userland constructors and serializers, DeviceId,
│                     query_permissions (stat), query_device_health (sysfs)
├── event.cyr       — DeviceEvent, DeviceEventKind (6), EventCollector,
│                     function pointer listener dispatch
├── storage.cyr     — Filesystem (17 types), mount/unmount/eject via syscalls,
│                     /proc/mounts parsing with octal unescape
├── optical.cyr     — DiscType (15), TrayState, DiscToc, tray control via ioctl,
│                     TOC reading, disc type detection
├── udev.cyr        — UdevEvent, UdevMonitor (netlink socket), device classification,
│                     sysfs enumeration, uevent parsing, partition discovery
├── linux.cyr       — LinuxDeviceManager (hashmap cache, listener dispatch,
│                     monitor lifecycle, mount/unmount/eject delegation)
├── udev_rules.cyr  — Rule rendering/validation, udevadm integration
├── partition.cyr   — MBR + GPT table reading, EFI System Partition detection,
│                     boot flag queries
├── device_db.cyr   — Persistent device history via patra (devices, mount_history,
│                     preferences, audio_devices tables) — gated off on agnos
├── network.cyr     — SMB/CIFS and NFS mount helpers, share detection via
│                     /proc/mounts and probing
├── gpu.cyr         — GPU discovery via /sys/class/drm/, GpuInfo (64 bytes),
│                     vendor/device/driver identification
├── audio.cyr       — ALSA PCM enumeration via /dev/snd/ + /proc/asound/,
│                     AudioDeviceInfo (72 bytes), vani descriptor adapter
├── lib.cyr         — Include chain (library entry point)
└── main.cyr        — CLI device enumeration demo
```

## Design Principles

1. **Zero dependencies** — all operations via direct Linux syscalls
2. **Bump + freelist allocation** — `alloc()` for long-lived data,
   `fl_alloc()`/`fl_free()` for DeviceEvent / UdevEvent lifetimes
3. **Manual struct layout** — fixed offsets, `store64`/`load64` accessors
4. **Enums as constants** — zero gvar_toks cost, compile-time values
5. **Function pointers** — replace trait objects for polymorphic dispatch
6. **Tagged unions** — `Ok(value)` / `Err(error)` for error handling
7. **Direct syscalls** — mount(165), umount2(166), ioctl(16), socket(41)
8. **sakshi logging** — structured logging on all operations

## Data Flow

### Device Enumeration
```
/sys/block/* → read_sysfs_attr() → build synthetic UdevEvent
  → classify_and_extract() → (DeviceClass, Capabilities)
  → device_info_from_udev() → DeviceInfo
  → populate LinuxDeviceManager hashmap cache
```

### Hotplug Monitoring
```
AF_NETLINK socket → ppoll() → recv() → parse_uevent()
  → UdevEvent → udev_event_to_device_event() → DeviceEvent
  → dispatch to listener function pointers
```

### Mount Operation
```
validate_mount_point() → mkdir(83) → mount(165, source, target, fstype, flags)
  → auto-detect: try ext4, vfat, ntfs, iso9660, udf, exfat, btrfs, xfs, f2fs, erofs
  → update LinuxDeviceManager cache (state=Mounted, mount_point=path)
```

## Struct Layouts

### DeviceInfo (168 bytes) — `src/core.cyr`
```
 0: id              8: dev_path        16: sys_path
24: class           32: state           40: label
48: vendor          56: model           64: serial
72: fs_type         80: mount_point     88: size_bytes
96: capabilities   104: detected_at    112: uid
120: gid           128: mode           136: usb_vendor_id
144: usb_product_id 152: partition_table 160: properties
```

### DeviceEvent (56 bytes) — `src/event.cyr`
```
 0: device_id       8: device_class    16: kind
24: dev_path        32: timestamp       40: extra
48: device_info
```

### UdevEvent (48 bytes) — `src/udev.cyr`
```
 0: action           8: sys_path        16: dev_path
24: subsystem       32: dev_type        40: properties
```

## Syscall Map

yukti issues no raw syscall numbers. Every kernel call goes through one of
three routes, and each resolves the number per target at compile time:

- a stdlib `sys_*` wrapper (`lib/syscalls*.cyr`);
- a portable `x*` / `file_*` helper (`lib/io.cyr`);
- a `_yk_*` bridge in `src/syscalls.cyr`, used where agnos lacks the
  wrapper or has a different ABI.

The one exception is `_yk_ppoll`: no stdlib peer wraps ppoll, so it is the
single `syscall()` in the tree. CI allows it there and nowhere else.

The x86_64 column below is for auditing (strace, seccomp allowlists). On
aarch64, cycc renumbers each call through ESYSXLAT.

| Operation | yukti calls | Linux wrapper | x86_64 | agnos |
|-----------|-------------|---------------|--------|-------|
| mount | `_yk_mount` | `sys_mount` | 165 | -ENOSYS (agnos `sys_mount` is a 0-arg stub) |
| unmount | `_yk_umount2` | `sys_umount2` | 166 | -ENOSYS |
| ioctl (optical, eject) | `_yk_ioctl` | `sys_ioctl` | 16 | -ENOSYS |
| socket (netlink, TCP probe) | `_yk_socket` | `sys_socket` | 41 | -ENOSYS |
| connect (TCP probe) | `_yk_connect` | `sys_connect` | 42 | -ENOSYS |
| bind (netlink) | `_yk_bind` | `sys_bind` | 49 | -ENOSYS |
| setsockopt (netlink) | `_yk_setsockopt` | `sys_setsockopt` | 54 | -ENOSYS |
| recvfrom (netlink) | `_yk_recvfrom` | `sys_recvfrom` | 45 | -ENOSYS |
| ppoll (netlink) | `_yk_ppoll` | — (raw; see above) | 271 | -ENOSYS |
| statfs (usage) | `_yk_statfs` | `sys_statfs` | 137 | statfs #103 |
| lstat (mount TOCTOU guard) | `_yk_lstat` | `sys_fstatat(AT_FDCWD, …, AT_SYMLINK_NOFOLLOW)` | 262 | lstat #102 |
| stat (permissions) | `xstat` | `sys_stat` | 4 | stat #33 |
| mkdir / rmdir / unlink | `xmkdir` / `xrmdir` / `xunlink` | `sys_mkdir` / `sys_rmdir` / `sys_unlink` | 83 / 84 / 87 | #9 / #10 / #30 |
| lseek (partition tables) | `xlseek` | — | 8 | #58 |
| open | `xopen` / `file_open` | `sys_open` | 2 | #7 (O_* → AO_*) |
| close / read / write | `sys_close` / `sys_read` / `sys_write` | same | 3 / 0 / 1 | #6 / #5 / #1 |
| clock_gettime | `clock_epoch_secs` (`lib/chrono.cyr`) | — | 228 | time_unix #46 |
| getdents64 | `dir_list` (`lib/fs.cyr`) | — | 217 | getdents #29 |

On agnos, the `x*` helpers and the statfs / lstat bridges pass an explicit
path length, because agnos path syscalls are `(path, pathlen, …)`.
