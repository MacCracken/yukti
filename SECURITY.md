# Security Policy

## Scope

Yukti is a Linux device abstraction layer, written in Cyrius, that interacts with hardware via sysfs, `/proc/mounts`, netlink sockets, and direct syscalls (mount, umount2, ioctl) — no libc, no FFI, no external dependencies. It requires elevated privileges for mount/eject operations.

## Attack Surface

| Area | Risk | Mitigation |
|------|------|------------|
| Mount syscall | Arbitrary filesystem mount | `validate_mount_point()` requires an absolute path, rejects `..` / `//` traversal, and blacklists system directories (`/`, `/usr`, `/etc`, `/boot`, etc.) and everything under them |
| Filesystem auto-detect | Trying multiple fs types against a device | Bounded list of 10 known types; stops on first match |
| `/proc/mounts` parsing | Crafted mount entries | Octal unescape is bounded; fields parsed by whitespace split |
| Sysfs attribute reading | Path traversal | Attributes read by literal name under a fixed `/sys` root; `udevadm` wrappers gated by `_is_sysfs_path()` |
| Netlink socket | Spoofed udev events | Kernel netlink group 1 (KOBJECT_UEVENT); `recvfrom` checks the source address and drops messages with `nl_pid != 0` |
| Ioctl calls | Privilege escalation | All ioctls use `O_RDONLY \| O_NONBLOCK`; permission errors mapped to `YUKTI_ERR_PERMISSION_DENIED` |
| USB eject via sysfs | Unintended device removal | Base device name allowlisted to `[a-zA-Z0-9_-]{1,32}` before composing `/sys/block/<dev>/device/delete`; no recursive paths |
| Device JSON output | Crafted device metadata | `device_info_to_json()` is emit-only — there is no JSON parser, and capabilities emit as an i64 bitmask |
| Event listener dispatch | Unbounded listener list | Listeners are function pointers in a caller-owned vec — consumer controls lifecycle |
| Device history (patra) | SQL injection via USB metadata | Every user-influenced field routed through `_sql_escape_str` before patra concat |

## Supported Versions

| Version | Supported |
|---------|-----------|
| 2.3.x | Yes |
| < 2.3 | No |

## Reporting a Vulnerability

Please report security issues to **security@agnos.dev**.

- You will receive acknowledgement within 48 hours
- We follow a 90-day coordinated disclosure timeline
- Please do not open public issues for security vulnerabilities

## Design Principles

- Cyrius has no `unsafe` construct — all memory access goes through `load8`/`load32`/`load64` and `store*` primitives with compiler-tracked widths; structs are `alloc()` plus named offset constants
- Kernel-safe subset (`src/core.cyr` + `src/pci.cyr`) carries zero alloc, zero syscalls, zero stdlib — `programs/core_smoke.cyr` is the CI tripwire
- Permission errors surface as a tagged `Err(YUKTI_ERR_PERMISSION_DENIED)`, never silently ignored
- No network I/O beyond the `network_probe_host()` TCP reachability check; SMB/NFS transport is the kernel's, via `mount(2)`
- Structured logging via `sakshi` — no ad-hoc prints
- Hardware- and root-dependent operations are excluded from the default test suite; their validation and bounds paths are covered instead
