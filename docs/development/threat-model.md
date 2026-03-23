# Threat Model

## Trust Boundaries

Yukti operates at the **library + kernel boundary**. It trusts the calling application to:
- Only mount/eject devices the user has authorized
- Install a `tracing` subscriber if logging is desired
- Manage listener lifecycle (avoid unbounded listener lists)

Yukti does NOT trust:
- Contents of `/proc/mounts` (parsed defensively with bounded field splitting)
- Sysfs attribute values (read as `Option<String>`, never unwrapped)
- Netlink uevent messages (parsed with fallback to `None` on malformed data)
- Filesystem type strings (case-insensitive comparison, no allocation for known types)

## Attack Surface

| Module | Risk | Mitigation |
|--------|------|------------|
| `storage` (mount) | Mounting on system directories | `validate_mount_point()` rejects `/`, `/usr`, `/boot`, `/dev`, `/sys`, `/proc`, `/etc`, `/bin`, `/sbin` |
| `storage` (mount) | Filesystem auto-detect probing | Bounded list of 8 known types; stops on first success |
| `storage` (mount) | Mount point path traversal | Must be absolute path; validated before `create_dir_all` |
| `storage` (unmount) | Unmounting arbitrary paths | Only cleans up dirs under `/run/media/` |
| `storage` (eject) | Sysfs write to wrong device | Writes only to `/sys/block/<basename>/device/delete` |
| `storage` (/proc/mounts) | Crafted mount entries | Octal unescape handles `\040`, `\011`, `\012`, `\134` only; unknown escapes pass through |
| `optical` (ioctl) | Privilege escalation | All ioctls use `O_RDONLY | O_NONBLOCK`; EPERM/EACCES mapped to `PermissionDenied` |
| `optical` (TOC) | Malformed TOC data | Track count bounded by kernel `CDROMREADTOCHDR`; LBA values clamped to `max(0)` |
| `udev` (netlink) | Spoofed uevent messages | Socket bound to kernel group 1 (KOBJECT_UEVENT); no udev daemon events |
| `udev` (parse_uevent) | Malformed uevent buffer | Null-byte split with `ACTION`/`DEVPATH`/`SUBSYSTEM` required; returns `None` on missing fields |
| `udev` (sysfs) | Path traversal via device name | Attributes read via `Path::join()` under a fixed sysfs root |
| `device` (serde) | Crafted JSON for `DeviceCapabilities` | Deserializes through `Vec<DeviceCapability>` enum validation; unknown variants rejected |
| `linux` (cache) | Stale device info | `refresh()` re-enumerates from sysfs; `enumerate()` clears cache |
| `event` (listeners) | Unbounded dispatch | Consumer controls listener count; class-based filtering reduces unnecessary dispatch |

## Unsafe Code

All `unsafe` blocks are in three categories:

1. **`libc` syscalls** — `mount()`, `umount2()`, `socket()`, `bind()`, `poll()`, `recv()`, `setsockopt()`, `close()`, `open()`, `ioctl()`
2. **FFI struct access** — `CdromAddr` union field access (`.lba`), `__errno_location()`
3. **File descriptor operations** — `OpenOptionsExt::custom_flags(O_NONBLOCK)`

Each block has a `// SAFETY:` comment. No pointer arithmetic, no manual memory management.

## Privilege Model

| Operation | Privilege Required |
|-----------|-------------------|
| Device enumeration (sysfs) | None |
| Mount point lookup (/proc/mounts) | None |
| Filesystem type detection | None |
| Device classification | None |
| Mount | Root or `CAP_SYS_ADMIN` |
| Unmount | Root or `CAP_SYS_ADMIN` |
| Eject (optical) | Root or device group membership |
| Eject (USB sysfs) | Root or sysfs write access |
| Tray control (ioctl) | Root or device group membership |
| TOC reading | Root or device group membership |
| Udev monitor (netlink) | `CAP_NET_ADMIN` on some kernels |

## Supply Chain

- `cargo-deny` enforces license allowlist, bans wildcards, denies unknown registries
- Minimal direct dependencies: serde, thiserror, tracing, chrono, uuid, bitflags
- `libc` gated behind features — not compiled when features are disabled
- Heavy dependencies (`reqwest`, `tokio`) only for `ai` feature
