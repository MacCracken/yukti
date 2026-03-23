# Security Policy

## Scope

Yantra is a Linux device abstraction layer that interacts with hardware via sysfs, `/proc/mounts`, netlink sockets, and `libc` syscalls (mount, umount2, ioctl). It requires elevated privileges for mount/eject operations.

## Attack Surface

| Area | Risk | Mitigation |
|------|------|------------|
| Mount syscall | Arbitrary filesystem mount | `validate_mount_point()` rejects system directories (`/`, `/usr`, `/boot`, etc.) |
| Filesystem auto-detect | Trying multiple fs types against a device | Bounded list of 8 known types; stops on first match |
| `/proc/mounts` parsing | Crafted mount entries | Octal unescape is bounded; fields parsed by whitespace split |
| Sysfs attribute reading | Path traversal | Attributes read via `Path::join()` under a fixed sysfs root |
| Netlink socket | Spoofed udev events | Kernel netlink group 1 (KOBJECT_UEVENT); no userspace daemon events |
| Ioctl calls | Privilege escalation | All ioctls use `O_RDONLY \| O_NONBLOCK`; permission errors mapped to `PermissionDenied` |
| USB eject via sysfs | Unintended device removal | Writes only to `/sys/block/<dev>/device/delete`; no recursive paths |
| Serde deserialization | Crafted JSON for `DeviceInfo` | Capabilities deserialize through `Vec<DeviceCapability>` enum validation |
| Event listener dispatch | Unbounded listener list | Listeners are `Arc<dyn EventListener>` — consumer controls lifecycle |

## Supported Versions

| Version | Supported |
|---------|-----------|
| 0.22.x | Yes |
| < 0.22 | No |

## Reporting a Vulnerability

Please report security issues to **security@agnos.dev**.

- You will receive acknowledgement within 48 hours
- We follow a 90-day coordinated disclosure timeline
- Please do not open public issues for security vulnerabilities

## Design Principles

- `unsafe` code limited to `libc` syscalls and ioctls — each block has a `// SAFETY:` comment
- All public types are `Send + Sync` where applicable
- Permission errors surface as `YantraError::PermissionDenied`, never silently ignored
- No network I/O in core library (`ai` feature is opt-in)
- Structured logging via `tracing` — no `println!`
- Hardware-dependent tests isolated with `#[ignore]`
