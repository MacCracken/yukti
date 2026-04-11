# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/MacCracken/yukti/compare/v0.22.3...HEAD
[0.22.3]: https://github.com/MacCracken/yukti/releases/tag/v0.22.3
