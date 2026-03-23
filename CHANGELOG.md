# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
