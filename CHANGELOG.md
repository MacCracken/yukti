# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Investigated, held

- **Native aarch64**. Cross-build succeeds with Cyrius 5.4.6's
  `cc5_aarch64`, but the produced binaries crash with `SIGILL`
  on real Cortex-A72 hardware (Raspberry Pi 4, Ubuntu 24.04).
  Faulting PC lands on word `0x800000d6` — an unallocated
  opcode in the ARMv8-A top-level encoding space. Affects every
  yukti target, including the minimal `core_smoke` (no stdlib,
  no syscalls). Diagnosed as a Cyrius compiler codegen bug, not
  a yukti issue. Held pending upstream fix. Reproducer filed at
  `docs/audit/2026-04-19-cc5-aarch64-repro.md`; one-command
  retest script at `scripts/retest-aarch64.sh`. The CI and
  release workflow hooks are in place and gated on
  `cc5_aarch64` existing — they stay dormant today and pick up
  automatically once the toolchain ships a fixed compiler.

## [2.1.0] — 2026-04-19

Follow-up release to close out the LOW findings from the 2.0.0
security audit (`docs/audit/2026-04-19-audit.md`) and ship the
near-term roadmap items. CHANGELOG is now the source of truth for
historical work — the roadmap has been trimmed to forward-looking
items only.

### Added
- **Dual-layer / dual-sided optical disc types**:
  `DT_DVD_WRITABLE_DL`, `DT_DVD_ROM_DL`, `DT_DVD_DUAL_SIDED`,
  `DT_BLURAY_DL`, `DT_BLURAY_XL`. Detection covers
  `dvd-r-dl`, `dvd+r-dl`, `dvd-r_dl`, `dvd-rom-dl`, `dvd-ds`,
  `dvd-dual-sided`, `bd-dl`, `bd-r-dl`, `bdxl`, `bd-xl` (case-insensitive).
  New `disc_type_nominal_sectors(dt)` returns the expected sector
  count per family (CD, DVD SL/DL/DS, BD SL/DL/XL) for display
  until the drive reports actual geometry.
- **Audio CD ripping** — `read_audio_sectors(dev, lba, nframes, buf, buflen)`
  wraps the `CDROMREADAUDIO` ioctl (2352-byte CD-DA frames, capped
  at 75 frames per call to bound latency). Higher-level
  `read_audio_track(dev, toc_entry, buf, buflen)` loops the per-
  call cap to rip a whole audio track.
- **Freelist plumbing** for DeviceEvent + UdevEvent — matching
  `device_event_free()` / `udev_event_free()`. Lets transient
  events from the hotplug dispatch loop be reclaimed. Investigation
  on `DeviceInfo` showed freelist overhead regresses the current
  long-lived call pattern by ~30%; kept on bump allocator with a
  no-op `device_info_free()` for API symmetry.
- **New test coverage** (+30 assertions, 562 → 592):
  `test_read_audio_sectors_rejects_bad_input`,
  `test_read_audio_track_rejects_data_track`,
  `test_detect_disc_type_layered`,
  `test_disc_type_nominal_sectors`,
  `test_disc_type_layered_predicates`,
  `test_validate_mount_point_trailing_slash`.
- **`docs/development/threat-model.md`** — full rewrite for the
  Cyrius era (was Rust leftover: `unsafe`, `cargo-deny`,
  `Option<String>`, `bitflags`). Covers trust boundaries, the
  17-row attack-surface matrix, privilege model, supply-chain
  stance, audit cadence, known gaps.

### Security (LOW-severity audit findings from 2.0.0)

- **[LOW-1] sysfs eject input allowlist**. `storage_eject` now
  validates the extracted device basename as `[a-zA-Z0-9_-]{1,32}`
  before composing `/sys/block/<name>/device/delete`. Defense-in-depth
  against path-traversal via crafted `dev_path` (sysfs legitimately
  uses symlinks under `/sys/block/`, so `O_NOFOLLOW` on the full
  path isn't applicable — we gate the untrusted component instead).
- **[LOW-2] TOC integer clamp**. `read_toc` clamps both track
  `length` and `leadout_lba` at 128 M sectors before any
  multiplication. A crafted disc can no longer produce nonsense
  duration / size values via adversarial TOC entries.
- **[LOW-3] Trailing-slash mount blacklist**. Already closed by
  the 2.0.0 MED-1 prefix-matching fix (`_starts_with_dir` treats
  trailing `/` as the component boundary). Regression test added.
- **[LOW-4] Mount-path label cap**. `default_mount_point`
  truncates the sanitized label at 64 chars to bound pathological
  USB labels and leave room under `PATH_MAX` for downstream file
  operations.
- **[LOW-5] Observability on malformed uevents**. `parse_uevent`
  emits `sakshi_warn` on empty `ACTION` / `DEVPATH` instead of
  silently returning 0 — a burst of these during an incident is
  a signal worth chasing.
- **[LOW-6] Threat model rewrite**. See Added section.

### Changed

- `disc_type_is_writable` now also returns true for
  `DT_DVD_WRITABLE_DL` so callers that gate burn UI behave
  correctly for dual-layer writable media.
- `disc_type_has_data` now covers every layered variant
  (DVD DL/DS, BD DL/XL) — previously missed the new variants.
- Roadmap stripped of completed items; CHANGELOG is the authoritative
  history. Remaining roadmap tracks Medium Term + Long Term only.

### Performance

Investigated the "targeted freelist" and "DeviceInfo pool for
enumerate" roadmap items. Findings:

- DeviceEvent (56 B) and UdevEvent (48 B) switched to `fl_alloc`
  with matching `_free` helpers. No bench regression in
  microsecond-resolution event paths; unlocks pool reuse for
  future consumers that track event lifecycle.
- DeviceInfo (168 B) kept on the bump allocator: matched bench
  showed a ~30% regression from fl_alloc overhead because the
  current call pattern (enumerate → cache → listener dispatch)
  has no `_free` call site to amortize against. Pool pays only
  under a churn workload that doesn't exist yet.
- `enumerate_devices` pooling deferred: same constraint —
  returned objects are consumed by long-lived callers. Will
  revisit when jalwa / argonaut expose a bulk-release API.

### Metrics

- **Tests**: 592 assertions (was 559, +33)
- **Source lines**: ~5270 (was ~5180)
- **Binary size**: ~350 KB static ELF (unchanged)
- **dist/yukti.cyr**: 5228 lines
- **dist/yukti-core.cyr**: 451 lines (unchanged — kernel-safe
  subset preserved through 2.x)
- **Fuzz targets**: 3 (unchanged from 2.0.0)

## [2.0.0] — 2026-04-19

First major version bump. 1.3.0 formalized the kernel-safe split
(`core.cyr` + `pci.cyr`) and multi-profile dist bundles; 2.0.0
follows up with a full P(-1) security audit pass, fixing every
HIGH and MEDIUM finding from `docs/audit/2026-04-19-audit.md`.

### Breaking

- **Stricter mount-path validation**. `validate_mount_point()` now
  rejects any path containing `..` or `//`, and the forbidden-root
  list matches both the root itself and everything under it
  (`/etc` + `/etc/foo`), plus new roots: `/var`, `/root`, `/home`,
  `/lib`, `/lib64`, `/srv`, `/opt`. Callers that previously relied
  on being able to mount under `/usr/local` or with un-canonical
  paths must pre-resolve.
- **GPT entry_size must be exactly 128 bytes**. `read_partition_table`
  now rejects GPT headers whose `entry_size` is not 128 (the
  single-size value produced by every real-world GPT writer). Disks
  with vendor extensions using larger entries will fail to parse —
  intentional; see HIGH-2.
- **`trigger_device` / `query_device` reject non-`/sys/` paths**.
  Previously the function accepted arbitrary paths and silently
  failed when udevadm rejected them; now returns `err_udev` on
  non-sysfs input before spawning any subprocess.
- **Udevadm wrappers now use absolute `/usr/bin/udevadm`**. Prior
  releases used a bare `"udevadm"` which `sys_execve` cannot resolve
  (no PATH lookup) — those code paths were effectively dead. Systems
  with udevadm only at `/sbin/udevadm` need to arrange for the
  `/usr/bin` symlink (modern distros already do).

### Security

All fixes below map 1:1 to findings in
`docs/audit/2026-04-19-audit.md`.

- **[HIGH-1] SQL injection via malicious USB descriptor fields** —
  `device_db.cyr` built every `patra_exec` / `patra_query` statement
  by string concatenation, allowing a USB stick advertising a
  crafted `ID_SERIAL` to tamper with the device-history DB.
  Introduced `_sql_escape_str` (doubles single quotes, drops NUL and
  newline bytes) and routed every user-influenced field (`key`,
  `vendor`, `model`, `dev_path`, `mount_point`, `fs_type`, `serial`)
  through it.
- **[HIGH-2] Stack buffer overflow via crafted GPT entry_size** —
  `_parse_gpt_entries` read `entry_size` bytes into a 128-byte stack
  buffer; a malicious disk setting `entry_size > 128` in the GPT
  header triggered stack corruption during partition scan. Parser
  now rejects any `entry_size != 128`.
- **[MED-1] Incomplete mount-point blacklist** — exact-match check
  missed `/var`, `/root`, `/home`, `/lib`, `/lib64`, `/srv`, `/opt`,
  and did not prefix-match (so `/etc/foo` was allowed). Replaced
  with `_starts_with_dir` prefix check over an extended list, plus
  a new `_path_has_traversal` gate that rejects `..` and `//`.
- **[MED-2] Mount TOCTOU (CVE-2026-27456 class)** — between
  `validate_mount_point` and `mount(2)` an attacker with write
  access to the mount parent could symlink the target. `storage_mount`
  now `newfstatat`s the final component with `AT_SYMLINK_NOFOLLOW`
  after `mkdir` and refuses to proceed if the target is a symlink
  or not a directory.
- **[MED-3] `/proc/mounts` truncation at 8 KB** — on container /
  btrfs / snap hosts, `/proc/mounts` exceeds 8 KB and
  `find_mount_point` silently returned false negatives.
  Reader now loops 4 KB chunks until EOF into a `str_builder`
  (capped at 1 MB as a DoS bound).
- **[MED-4] Netlink uevent spoofing (CVE-2009-1185 class)** —
  `udev_monitor_poll` called `recvfrom` with NULL `src_addr`, so
  kernel-origin was never verified. Now passes `sockaddr_nl`,
  checks `nl_pid == 0`, and drops messages with non-zero pid
  (defense-in-depth on pre-hardening kernels).
- **[MED-5] Broken `run("udevadm", ...)` pattern** — existing
  wrappers passed multi-token arg strings into a single argv slot
  and used a relative command name that `sys_execve` cannot
  resolve. Every udevadm caller now builds an argv vec with
  `/usr/bin/udevadm` as absolute cmd and one token per element,
  via `exec_vec` / `exec_capture`. Closes a latent command-injection
  surface that would have opened if `run()` ever grew shell support.

### Added

- `docs/audit/2026-04-19-audit.md` — full P(-1) audit report:
  methodology, 13 findings with file/line references, CVE sweep
  of 10 adjacent kernel/util-linux/udev classes, remediation plan.
- `fuzz/fuzz_partition_table.fcyr` — closes the audit-flagged
  coverage gap: MBR + GPT parser fuzzing via temp-file fixture,
  500 mutation rounds + truncation pass, explicit HIGH-2
  regression check (malicious entry_size must be rejected).
- 28 new test assertions for the security fixes:
  `test_validate_mount_point_blacklist_extended`,
  `test_validate_mount_point_rejects_traversal`,
  `test_sql_escape`, `test_udevadm_sysfs_path_gate`.

### Changed

- CI release and main workflows rewritten for 5.4.6 toolchain +
  multi-dist + kernel-safe tripwire (see 1.3.0 entry).
- `docs/development/roadmap.md` reorganized — LOW audit findings
  scheduled for 2.1.0; `Future` section retained verbatim.

### Metrics

- **Tests**: 559 assertions (was 531, +28 security regressions)
- **Fuzz targets**: 3 (was 2; added `fuzz_partition_table`)
- **Binary size**: ~348 KB static ELF (unchanged)
- **Source lines**: ~5180 (was 5067)
- **dist/yukti.cyr**: 5147 lines
- **dist/yukti-core.cyr**: 451 lines (unchanged — kernel-safe subset
  untouched by security fixes, which is by design)

### Non-findings (verified clean during audit)

- No memory-unsafe primitives in user code (Cyrius has no raw
  pointer arithmetic).
- No libc / FFI.
- No `sys_system()` in `src/`.
- No raw `execve(59)` / `fork(57)` in `src/` (only in audited
  `lib/process.cyr` stdlib).
- No writes to `/etc`, `/bin`, `/sbin`.
- Kernel-safe invariant verified — `dist/yukti-core.cyr` contains
  zero `alloc` / `syscall` / `sys_*` references.

## [1.3.0] — 2026-04-19

### Added
- **`core.cyr`** — kernel-safe core types extracted from `device.cyr`:
  `DeviceClass`, `DeviceState`, `DeviceCapabilities`, struct layouts,
  pure accessors/predicates. Zero alloc, zero syscalls, zero stdlib —
  safe for bare-metal consumption by the AGNOS kernel for PCI device
  identification.
- **`pci.cyr`** — kernel-safe PCI class/subclass/vendor/device tables
  and pure predicates (`pci_class_to_device_type`, `pci_is_storage`,
  `pci_is_nvme`, `pci_is_gpu`, `pci_is_network`, etc.). Same
  kernel-safe discipline as `core.cyr`.
- **`programs/core_smoke.cyr`** — invariant check for the kernel-safe
  subset. Links only `core.cyr` + `pci.cyr` (no `src/lib.cyr`,
  no stdlib) and asserts every exported predicate. Tripwire for
  accidental alloc/syscall additions to the kernel-safe modules.
- **Multi-dist profiles** (requires Cyrius 5.4.6+):
  - `cyrius distlib` → `dist/yukti.cyr` (full userland, 4929 lines)
  - `cyrius distlib core` → `dist/yukti-core.cyr` (kernel-safe, 451 lines)
  - `[lib.core]` section in `cyrius.cyml` declares the profile
- **`docs/development/cyrius-usage.md`** — single source of truth for
  toolchain commands (build, test, bench, fuzz, distlib, deps, release),
  multi-profile dist bundles, quality gates, and Yukti-relevant Cyrius
  conventions. Referenced from `CLAUDE.md`.
- PCI class/vendor lookup tests added to `tests/tcyr/yukti.tcyr`

### Changed
- **Toolchain pin**: `cyrius.cyml` now requires Cyrius 5.4.6+
  (was 5.2.1). Needed for multi-dist profile support (`[lib.PROFILE]`).
- **`CLAUDE.md` restructured** to match the agnosticos first-party
  application template (`docs/development/applications/example_claude.md`
  in the agnosticos repo). Sections now align across AGNOS projects:
  Project Identity, Goal, Current State, Consumers, Dependencies,
  Quick Start, Architecture, Key Constraints, Development Process
  (P(-1) + Work Loop + Security Hardening + Closeout), Key Principles,
  CI/Release, Key References, DO NOT.
- Toolchain-specific commands moved out of `CLAUDE.md` into
  `docs/development/cyrius-usage.md`; `CLAUDE.md` now links there
  instead of duplicating.

### Fixed
- `cyrius fmt --check` now diff-clean across `src/`, `programs/`,
  `tests/`, `fuzz/` (3 files re-formatted: `core_smoke.cyr`,
  `tests/tcyr/yukti.tcyr`, `tests/bcyr/yukti.bcyr`).
- `cyrius lint` now reports 0 warnings across the whole project.
  Previously silent byte-length overflows (Unicode box-drawing chars
  in bench section headers counted as 3 bytes each), duplicate blank
  lines in 7 domain modules, and long one-liner bench declarations.
- `cyrius vet src/main.cyr` clean (1 dep, 0 untrusted, 0 missing).

### Metrics
- **Modules**: 16 (was 14 — added `core.cyr`, `pci.cyr`)
- **Source lines**: 5067 (was 4573)
- **Tests**: 531 assertions (was 485)
- **Binary size**: ~348 KB static ELF
- **Full dist bundle**: 4929 lines (`dist/yukti.cyr`)
- **Kernel-safe dist bundle**: 451 lines (`dist/yukti-core.cyr`)

### Consumers
- AGNOS kernel now consumes `dist/yukti-core.cyr` for PCI device
  identification — same tables userland uses, zero runtime cost.

## [1.2.0] — 2026-04-11

### Added
- **`gpu.cyr`** — GPU device discovery via `/sys/class/drm/`. Enumerates GPU
  devices, reads PCI vendor/device IDs, driver name, and PCI slot from sysfs.
  Known vendors: AMD (0x1002), Intel (0x8086), NVIDIA (0x10DE), VirtIO (0x1AF4).
  API: `enumerate_gpus()`, `gpu_count()`, `gpu_info_report(g)`, plus accessors
  for card name, dev path, sys path, vendor/device ID, driver, PCI slot, and
  render node. Unblocks mabda GPU library port (pre-flight GPU detection).
- **`DC_GPU`** device class added to `DeviceClass` enum (value 7, `DC_UNKNOWN` → 8)

## [1.1.2] — 2026-04-11

### Fixed
- All source files pass `cyrfmt` (indentation, line wrapping)
- All source files pass `cyrlint` (0 warnings — no double blank lines, no lines >100 chars)
- Bundle script now strips consecutive blank lines automatically
- SQL string literals split to stay under 100-char line limit

## [1.1.1] — 2026-04-11

### Fixed
- All private helper functions consistently prefixed with `_` (storage, optical, udev, partition, device_db, network)
- Removed duplicate `str_to_hex()` and `str_to_int()` from udev.cyr (now provided by lib/str.cyr)
- Added inline doc comments to all accessor functions (partition, network, device_db)
- Zero compiler warnings on clean build

## [1.1.0] — 2026-04-11

### Added
- **`partition.cyr`** — MBR and GPT partition table reading
  - MBR: 4 primary entries, 15 known type IDs, boot flag (0x80)
  - GPT: header validation ("EFI PART"), 128-byte entries, mixed-endian GUID formatting
  - 4 known GPT type GUIDs: EFI System, Linux filesystem, Linux swap, Microsoft Basic Data
  - `read_partition_table(dev)`, `find_efi_partition()`, `find_bootable_partitions()`
  - `partition_count()`, `has_efi_partition()`, `read_partition_table_by_name()`
- **`device_db.cyr`** — device database persistence via patra
  - 3 tables: `devices` (known devices), `mount_history` (event log), `preferences` (per-device config)
  - `device_db_record_seen()`, `device_db_record_mount()`, `device_db_is_known()`
  - `device_db_set_preference()` / `device_db_get_preference()` — per-device mount config
  - `device_db_mount_count()`, `device_db_device_count()`
- **`network.cyr`** — network filesystem mount helpers
  - SMB/CIFS and NFS/NFS4 mount via direct syscall with credential and port support
  - `NetworkShare` struct — host, path, fs_type, port, username, password
  - `network_mount()`, `network_unmount()`, `network_list_mounted()`
  - `network_probe_smb()` / `network_probe_nfs()` — TCP connect probe on ports 445/2049
  - `network_mount_source()` — builds `//host/path` (SMB) or `host:/path` (NFS)
- **sakshi_full structured logging** — upgraded from minimal sakshi
  - Span-based instrumentation on mount/unmount/eject, tray control, TOC reading, enumeration, udev monitor
  - `sakshi_span_enter()` / `sakshi_span_exit()` with automatic duration tracking

### Changed
- Include chain (`lib.cyr`) now uses `sakshi_full.cyr` instead of `sakshi.cyr`
- Added `lib/patra.cyr` and `lib/freelist.cyr` as stdlib dependencies
- Bundle script updated to include partition, device_db, network modules
- CI/release workflows updated for Cyrius toolchain (matching patra/sakshi pattern)
- Makefile rewritten for Cyrius build/test/bench/fuzz targets

### Metrics
- **Modules**: 13 (was 10)
- **Source lines**: 4,573 (was 3,359)
- **Tests**: 470 assertions (was 407)
- **Binary size**: 307 KB (was 152 KB — includes patra SQL engine)
- **dist bundle**: 4,477 lines

## [1.0.0] — 2026-04-11

### Changed — **Cyrius Port**
- **Complete rewrite from Rust to Cyrius** — sovereign, zero-dependency implementation
- All 8 modules ported: error, device, event, storage, optical, udev, linux, udev_rules
- Direct Linux syscalls replace libc wrappers: mount(165), umount2(166), ioctl(16), socket(41), stat(4)
- Function pointer callbacks replace Rust trait objects for event listeners
- Manual struct layout with alloc/store64/load64 replaces Rust structs
- Enum integer constants replace Rust enums with derives
- Tagged union Ok/Err replaces Rust Result<T, E>
- sakshi structured logging replaces tracing crate
- Bump allocator replaces Rust ownership/borrowing

### Added
- `src/main.cyr` — CLI device enumeration demo (prints device table)
- `tests/yukti.tcyr` — **407 test assertions** (up from 229 in Rust)
- `benches/bench.bcyr` — 45 batch-timed benchmarks with nanosecond precision
- `fuzz/fuzz_parse_uevent.fcyr` — 1000 mutations + truncation fuzzing for uevent parser
- `fuzz/fuzz_mount_table.fcyr` — 500 mutations + truncation fuzzing for mount table parser
- `BENCHMARKS-rust-v-cyrius.md` — comprehensive Rust vs Cyrius performance comparison
- Extended `lib/str.cyr` with: `str_from_buf`, `str_eq_cstr`, `str_cstr`, `str_substr`, `str_last_index_of`, `str_builder_add_str`, `str_builder_add_byte`, `str_contains_cstr`, `str_index_of_cstr`, `str_to_hex`
- `WIFSIGNALED` and `WTERMSIG` macros to `lib/syscalls.cyr`

### Metrics
- **Binary size**: 152 KB static ELF (vs 449 KB Rust stripped)
- **Source**: 3,359 lines (vs 6,166 Rust)
- **Dependencies**: 0 (vs 47 Rust crates)
- **Tests**: 407 assertions, 0 failures
- **Benchmarks**: 45 operations, batch-timed
- **Fuzz targets**: 2 (parse_uevent, mount_table)

### Archived
- Original Rust source moved to `rust-old/`
- Cargo.toml, Cargo.lock, deny.toml, rust-toolchain.toml archived
- Criterion benchmarks archived as `rust-old/yukti_bench.rs`
- libfuzzer targets archived as `rust-old/fuzz/`

## [0.25.3] — 2026-03-25

### Added
- **`udev-rules` feature**: udev rule management via agnosys integration
  - `udev_rules` module with `render_rule()`, `validate_rule()`, `write_rule()`, `remove_rule()`, `reload_rules()`
  - `trigger_device()`, `query_device()`, `list_devices()` via udevadm
  - Feature-gated behind `dep:agnosys` (optional git dependency, not in `full` or `default`)
  - 13 unit tests
- `full` feature combining `udev`, `storage`, `optical`, `ai`

### Changed
- CI: `--all-features` replaced with `--features full` to avoid requiring private path dependencies
- `deny.toml`: switched to `features = ["full"]`, allow agnosys git source
- Release workflow: strip private deps before `cargo publish`

## [0.22.3] — 2026-03-22

### Added
- `device` module: `DeviceInfo`, `DeviceId`, `DeviceClass` (8 types), `DeviceState`, `Device` trait
- `DeviceCapabilities` bitflags (u16) replacing `Vec<DeviceCapability>` — O(1) membership checks with serde compatibility (serializes as array)
- `DeviceCapability` enum (10 variants) with `.flag()` conversion to bitflags
- `DeviceInfo::display_name()` returns `Cow<str>` (zero-alloc for label/model paths)
- `DeviceInfo::size_display()` returns `Cow<'static, str>` (zero-alloc for "unknown")
- `event` module: `DeviceEvent`, `DeviceEventKind`, `EventListener` trait, `EventCollector` (thread-safe)
- `EventCollector::with_events()` — zero-copy event access via closure
- `storage` module: `Filesystem` enum (12 types) with zero-allocation case-insensitive parsing via `eq_ignore_ascii_case`
- `MountOptions` with builder pattern: `new()`, `mount_point()`, `read_only()`, `fs_type()`, `option()`
- `MountResult`, mount point validation (direct `Path` comparison, no string allocation)
- `find_mount_point()` — parses `/proc/mounts` with octal unescape, testable via internal `find_mount_in()` helper
- `mount()` — `libc::mount()` with auto-detect filesystem, already-mounted check
- `unmount()` — `libc::umount2()` with mount point cleanup under `/run/media/`
- `eject()` — optical via `CDROMEJECT` ioctl (RAII fd guard), USB via sysfs `device/delete`, nvme/mmcblk-aware
- `optical` module: `DiscType` (10 types), `TrayState`, `DiscToc`, `TocEntry`, `TrackType`
- `detect_disc_type()` — zero-allocation case-insensitive media type classification
- `open_tray()`, `close_tray()`, `drive_status()` — optical drive ioctl wrappers
- `read_toc()` — reads CD TOC via `CDROMREADTOCHDR`/`CDROMREADTOCENTRY`, computes track lengths and durations (75 frames/sec)
- `udev` module: `UdevEvent` parsing, `classify_device()`, `extract_capabilities()`, `classify_and_extract()` (single-pass)
- `device_info_from_udev()` — builds `DeviceInfo` from udev properties
- `enumerate_devices()` — walks sysfs, builds `DeviceInfo` for disks and partitions
- `UdevMonitor` — netlink socket (`AF_NETLINK`/`NETLINK_KOBJECT_UEVENT`), `poll()`, `run_with_listener()`, `subscribe()` channel API
- `parse_uevent()` — kernel uevent message parser (null-separated key=value)
- `linux` module: `LinuxDeviceManager` implementing `Device` trait with `Arc<DeviceInfo>` cache
- `LinuxDeviceManager::mount()`, `unmount()`, `eject()` by device ID with state tracking
- `LinuxDeviceManager::start_monitor()` / `stop_monitor()` — background hotplug monitoring
- `LinuxDeviceManager::dispatch_event()` — listener dispatch with class-based filtering
- `error` module: `YuktiError` with 15 variants including `AlreadyMounted`, `Timeout`, `UdevSocket`, `UdevParse`
- `From<&str>` and `From<String>` for `DeviceId` and `Filesystem`
- `#[non_exhaustive]` on all 9 public enums
- `tracing` instrumentation on all I/O operations (mount, unmount, eject, tray, monitor, enumerate)
- RAII `OwnedFd` guard for fd management in ioctl paths, named `ENOMEDIUM` constant
- Safe errno access via `std::io::Error::last_os_error()` (no unsafe errno)
- Feature gates: `udev`, `storage`, `optical` (all require `libc`), `ai` (requires `reqwest`, `tokio`)
- Criterion benchmarks: 45 benchmarks across 9 groups with 3-point history tracking
- `scripts/bench-history.sh`, `scripts/version-bump.sh`
- GitHub Actions CI + release workflows, Makefile, deny.toml, codecov.yml
- `docs/`: architecture overview, threat model, roadmap, testing guide
- CONTRIBUTING.md, CODE_OF_CONDUCT.md, SECURITY.md
- `examples/detect.rs` — device detection, filesystem parsing, disc type detection
- 175 tests (12 hardware tests `#[ignore]`d), clippy clean with `-D warnings`

[Unreleased]: https://github.com/MacCracken/yukti/compare/v2.1.0...HEAD
[2.1.0]: https://github.com/MacCracken/yukti/releases/tag/v2.1.0
[2.0.0]: https://github.com/MacCracken/yukti/releases/tag/v2.0.0
[1.3.0]: https://github.com/MacCracken/yukti/releases/tag/v1.3.0
[1.2.0]: https://github.com/MacCracken/yukti/releases/tag/v1.2.0
[1.1.2]: https://github.com/MacCracken/yukti/releases/tag/v1.1.2
[1.1.1]: https://github.com/MacCracken/yukti/releases/tag/v1.1.1
[1.1.0]: https://github.com/MacCracken/yukti/releases/tag/v1.1.0
[1.0.0]: https://github.com/MacCracken/yukti/releases/tag/v1.0.0
[0.25.3]: https://github.com/MacCracken/yukti/releases/tag/v0.25.3
[0.22.3]: https://github.com/MacCracken/yukti/releases/tag/v0.22.3
