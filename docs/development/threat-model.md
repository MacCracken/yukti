# Threat Model — Yukti (Cyrius era, 2.1.0)

Yukti is a device-abstraction library. It reads kernel surfaces
(sysfs, `/proc/mounts`, netlink uevents, CDROM ioctls, raw block
devices for MBR/GPT), issues mount / umount / eject on behalf of
callers, and records device history in a `patra` store.

It runs in two modes:

- **Userland library** — linked into jalwa, argonaut, aethersafha,
  the AGNOS file manager. All syscalls direct; no libc.
- **Kernel-safe subset** — `dist/yukti-core.cyr` contains only
  `src/core.cyr` + `src/pci.cyr`: pure enums, struct layouts, PCI
  class/vendor tables. Zero alloc, zero syscalls, zero stdlib.
  The AGNOS kernel links this bundle bare-metal for PCI device
  identification.

For the full audit history see `docs/audit/`. This document
records the threats Yukti defends against, the assumptions we
make, and the surfaces where those defences are known to be
incomplete.

## Trust Boundaries

Yukti trusts the caller to:

- Hold `CAP_SYS_ADMIN` (or root) for mount / unmount / eject.
  Enumeration and classification require none.
- Manage its own listener lifecycle (avoid unbounded listener lists
  fed into `event_collector_push`).
- Call `sakshi_init` if structured logging is desired. Yukti emits
  spans and warn/info events; the sink is the caller's concern.
- Hold the sole reference to the patra DB file during a session
  (patra acquires its own `flock`; concurrent yukti processes
  sharing a DB must coordinate).

Yukti explicitly does **not** trust:

- Contents of `/proc/mounts` — parsed defensively with bounded
  field splitting and octal unescape. Read in 4 KB chunks up to
  1 MB; truncation under a hostile `/proc/mounts` becomes a
  visible "mount not found" result, never a buffer overrun.
- Sysfs attribute values — read as `Str`; no struct that consumes
  them dereferences a missing / zero value without a null check.
- Netlink uevent messages — parsed null-separated with `ACTION`
  and `DEVPATH` required; parser discards malformed messages with
  a `sakshi_warn`. The monitor verifies `nl_pid == 0` (kernel
  origin) on every packet.
- Raw disk sectors for MBR / GPT — signature checked, entry sizes
  clamped (GPT `entry_size` must be exactly 128; `num_entries`
  capped at `GPT_MAX_ENTRIES=128`), integer fields widened to i64
  before arithmetic.
- CDROM TOC ioctl output — track lengths and lead-out LBA clamped
  to 128 M sectors before any multiplication.
- Filesystem-type strings — case-insensitive compare against a
  fixed set of 17 known types; no allocation for known variants.
- USB descriptor fields (`ID_SERIAL`, `ID_VENDOR`, `ID_MODEL`,
  `ID_FS_LABEL`) — these flow into mount paths and the device DB
  and must never be interpolated into SQL unescaped.

## Attack Surface

| Module            | Risk                                          | Mitigation                                                                                            |
|-------------------|-----------------------------------------------|-------------------------------------------------------------------------------------------------------|
| `storage` (mount) | Mounting over system directories              | `_is_forbidden_mount` prefix-matches `/` `/bin` `/sbin` `/lib` `/lib64` `/usr` `/etc` `/boot` `/sys` `/proc` `/dev` `/root` `/var` `/home` `/srv` `/opt` |
| `storage` (mount) | Path traversal via `..` / `//`                | `_path_has_traversal` rejects both in `validate_mount_point`                                          |
| `storage` (mount) | Symlink TOCTOU (CVE-2026-27456 class)         | `newfstatat(AT_SYMLINK_NOFOLLOW)` after `mkdir` — refuses to proceed if target is a symlink           |
| `storage` (mount) | FS-type auto-detect probing                   | Bounded list of 10 known types; stops on first success                                                |
| `storage` (unmount) | Unmounting arbitrary paths                  | Caller-supplied path; consumers are expected to restrict to `/run/media/` (yukti does not re-validate) |
| `storage` (eject) | Sysfs write to crafted device name           | `base_name` validated as `[a-zA-Z0-9_-]{1,32}` before composing `/sys/block/<name>/device/delete`     |
| `storage` (/proc/mounts) | Crafted mount entries                 | Chunked read up to 1 MB; octal unescape handles `\NNN` only; unknown escapes pass through             |
| `storage` (labels)| Pathological USB label bloats mount path      | Sanitized label capped at 64 chars in `default_mount_point`                                           |
| `optical` (ioctl) | Privilege escalation via crafted disc         | All ioctls use `O_RDONLY \| O_NONBLOCK`; EPERM/EACCES mapped to `PermissionDenied`                    |
| `optical` (TOC)   | Malformed TOC, overflow on track lengths      | Track count bounded by `CDROMREADTOCHDR`; length + leadout_lba clamped to 128 M sectors               |
| `udev` (netlink)  | Spoofed uevents (CVE-2009-1185 class)         | Socket bound to kernel group 1; `recvfrom` reads src_addr and drops messages with `nl_pid != 0`      |
| `udev` (parse_uevent) | Malformed uevent buffer                  | Null-byte split; `ACTION`/`DEVPATH` required; fuzzed (`fuzz/fuzz_parse_uevent.fcyr`)                 |
| `udev` (sysfs)    | Path traversal via device name                | Attributes read under a fixed `/sys` root; `_is_sysfs_path` gate on `trigger_device` / `query_device`|
| `udev_rules`      | Command injection via crafted syspath         | `exec_vec` with absolute `/usr/bin/udevadm` + one argv element per token; sysfs-prefix gate          |
| `device`          | USB fields reflected into paths / DB          | Sanitized before mount path composition; `_sql_escape_str` before every patra SQL interpolation      |
| `partition`       | GPT stack overflow via crafted `entry_size`   | `read_partition_table` rejects `entry_size != 128`; `num_entries` clamped at 128; fuzzed (`fuzz/fuzz_partition_table.fcyr`) |
| `partition`       | MBR out-of-bounds reads                       | Fixed 4 primary entries × 16-byte stride over sector 0                                                |
| `device_db`       | SQL injection via USB metadata                | Every user-influenced field routed through `_sql_escape_str` before patra concat                     |
| `linux`           | Stale device info                             | `refresh()` re-enumerates from sysfs; `enumerate()` clears cache                                      |
| `event`           | Unbounded listener dispatch                   | Consumer controls listener count; class-based filtering                                               |
| `network`         | SMB/NFS mount credential handling             | Credentials passed to kernel mount(2) via options string; never logged by sakshi at `info` level     |

## Unsafe Operations (Cyrius)

Cyrius has no `unsafe` keyword — all memory access is through
`load8`/`load16`/`load32`/`load64` and `store*` primitives with
compiler-tracked widths. There is no pointer arithmetic that the
compiler can't see. Yukti adds no FFI and no libc. The trust
surfaces that matter:

1. **Raw syscalls** — every `syscall(N, …)` has its return checked
   at the call site; destructive syscalls (mount 165, umount2 166,
   ioctl 16, socket 41, execve 59, write 1 to sysfs) are reviewed
   individually. Arity warnings from the compiler are errors.
2. **Struct layout** — `alloc(N)` + `store64` with named offset
   constants. Offsets are declared as enum values per-module,
   not reused across modules.
3. **Stack buffers** — `var buf[N]` allocates N bytes on the
   frame. CI security scan rejects any declaration ≥ 64 KB
   (`.github/workflows/ci.yml` security job).

## Privilege Model

| Operation                    | Privilege Required                        |
|------------------------------|-------------------------------------------|
| Device enumeration (sysfs)   | None                                      |
| Mount point lookup (/proc/mounts) | None                                 |
| Filesystem type detection    | None                                      |
| Device classification        | None                                      |
| PCI class/vendor lookup      | None (kernel-safe subset)                 |
| Mount / unmount              | Root or `CAP_SYS_ADMIN`                   |
| Eject (optical, CDROMEJECT)  | Root or device group membership           |
| Eject (USB sysfs)            | Root (sysfs write requires uid=0)         |
| Tray control / TOC           | Root or device group membership           |
| Udev monitor (netlink group 1) | Normally none on modern kernels; `CAP_NET_ADMIN` required on some older kernels |

## Supply Chain

- No external runtime dependencies. No libc, no FFI, no dynamic
  link surface.
- `cyrius.lock` records SHA-256 hashes of every resolved
  `lib/*.cyr`. CI runs `cyrius deps --verify` on every build.
- First-party deps only — `sakshi 2.0.0` (logging), `patra 1.1.1`
  (embedded store). Both share the Yukti threat model and are
  audited on the same cadence.
- Cyrius stdlib (`alloc`, `str`, `vec`, `hashmap`, `io`, `fs`,
  `process`, etc.) ships with the toolchain release (5.5.11) and
  is SHA-pinned by the toolchain installer, not by yukti.

## Audit Cadence

- **P(-1) pass before every major release** — fmt/lint/vet/test/
  bench + CVE sweep of kernel / util-linux / udev / GPT surfaces
  for the last 24 months. Findings filed under `docs/audit/YYYY-MM-DD-audit.md`.
- **Security gate on every CI run** — raw `execve` / `fork` /
  `sys_system` / writes to `/etc` `/bin` `/sbin` / stack buffers
  ≥ 64 KB fail the build.
- **Fuzz gate on every CI run** — `fuzz_parse_uevent`,
  `fuzz_mount_table`, `fuzz_partition_table` must run clean.
- **Kernel-safe tripwire on every CI run** — `programs/core_smoke.cyr`
  links only `core.cyr` + `pci.cyr` and asserts every exported
  kernel-safe constant/predicate. Any alloc/syscall that leaks
  into the kernel-safe subset fails the build.

## Out of Scope

- Offline decryption of LUKS-encrypted volumes.
- Firmware-level attacks (BadUSB HID injection, controller-level
  replay). Yukti runs on top of the kernel's USB stack and relies
  on it for device enumeration.
- Physical attacks that bypass `/sys/block/<name>/device/delete`
  (yanking the cable).
- Kernel CVEs themselves — yukti is a consumer of kernel
  interfaces; we defend against misuse of those interfaces, not
  against the kernel providing compromised data. If the kernel
  is owned, yukti is owned.

## Known Gaps

- **No mount point canonicalisation.** We reject `..` and `//`
  but do not resolve symlinks in the parent path before
  `validate_mount_point`. A caller who constructs the mount
  target from attacker-controlled components must pre-canonicalise
  or live with the TOCTOU guard at `newfstatat` time.
- **No `open_tree`/`move_mount` path.** Linux 5.2+ offers
  `open_tree(2)` + `move_mount(2)` for atomic fd-based mount
  target pinning. We use the older `mount(2)` + `newfstatat`
  pair; a determined attacker with write access to the mount
  parent can still race the stat and the mount, though the
  window is small.
- **Patra is not prepared-statement-capable.** We escape strings
  defensively (`_sql_escape_str`) but the surface-area argument
  is weaker than parameterised queries. If patra grows a
  `patra_bind_*` API, we should switch.
- **No `lib/` dep audit gate.** First-party deps are tag-pinned
  in `cyrius.cyml` and SHA-locked in `cyrius.lock`, but we do
  not re-audit sakshi or patra on the yukti cadence. Each
  library audits itself.
