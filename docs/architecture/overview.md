# Architecture Overview

## Module Map

```
yukti (Cyrius)
├── error.cyr       — 16 error kinds, heap-allocated error structs, errno mapping
├── device.cyr      — DeviceInfo (168 bytes, 21 fields), DeviceId, DeviceClass (8),
│                     DeviceCapabilities (bitflags), DeviceHealth, query_permissions
├── event.cyr       — DeviceEvent, DeviceEventKind (6), EventCollector,
│                     function pointer listener dispatch
├── storage.cyr     — Filesystem (17 types), mount/unmount/eject via syscalls,
│                     /proc/mounts parsing with octal unescape
├── optical.cyr     — DiscType (10), TrayState, DiscToc, tray control via ioctl,
│                     TOC reading, disc type detection
├── udev.cyr        — UdevEvent, UdevMonitor (netlink socket), device classification,
│                     sysfs enumeration, uevent parsing, partition discovery
├── linux.cyr       — LinuxDeviceManager (hashmap cache, listener dispatch,
│                     monitor lifecycle, mount/unmount/eject delegation)
├── udev_rules.cyr  — Rule rendering/validation, udevadm integration
├── lib.cyr         — Include chain (library entry point)
└── main.cyr        — CLI device enumeration demo
```

## Design Principles

1. **Zero dependencies** — all operations via direct Linux syscalls
2. **Bump allocation** — `alloc()` for all heap data, no free list
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
AF_NETLINK socket → poll() → recv() → parse_uevent()
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

### DeviceInfo (168 bytes)
```
 0: id              8: dev_path        16: sys_path
24: class           32: state           40: label
48: vendor          56: model           64: serial
72: fs_type         80: mount_point     88: size_bytes
96: capabilities   104: detected_at    112: uid
120: gid           128: mode           136: usb_vendor_id
144: usb_product_id 152: partition_table 160: properties
```

### DeviceEvent (56 bytes)
```
 0: device_id       8: device_class    16: kind
24: dev_path        32: timestamp       40: extra
48: device_info
```

### UdevEvent (48 bytes)
```
 0: action           8: sys_path        16: dev_path
24: subsystem       32: dev_type        40: properties
```

## Syscall Map

| Operation | Syscall | Number |
|-----------|---------|--------|
| mount | SYS_MOUNT | 165 |
| unmount | SYS_UMOUNT2 | 166 |
| ioctl (optical) | SYS_IOCTL | 16 |
| socket (netlink) | SYS_SOCKET | 41 |
| bind | SYS_BIND | 49 |
| poll | SYS_POLL | 7 |
| recv | SYS_RECVFROM | 45 |
| stat (permissions) | SYS_STAT | 4 |
| statfs (usage) | SYS_STATFS | 137 |
| mkdir | SYS_MKDIR | 83 |
| rmdir | SYS_RMDIR | 84 |
| open/close/read/write | 2/3/0/1 | — |
| clock_gettime | 228 | — |
| getdents64 (dir_list) | 217 | — |
