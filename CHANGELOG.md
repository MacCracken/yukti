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
- `storage` module: `Filesystem` enum (12 types) with zero-allocation case-insensitive parsing via `eq_ignore_ascii_case`
- `MountOptions`, `MountResult`, mount point validation (direct `Path` comparison, no string allocation)
- `find_mount_point()` — parses `/proc/mounts` with octal unescape, testable via internal `find_mount_in()` helper
- `mount()` — `libc::mount()` with auto-detect filesystem (tries ext4, vfat, ntfs, iso9660, udf, exfat, btrfs, xfs)
- `unmount()` — `libc::umount2()` with mount point cleanup under `/run/media/`
- `eject()` — optical via `CDROMEJECT` ioctl, USB via sysfs `device/delete`
- `optical` module: `DiscType` (10 types), `TrayState`, `DiscToc`, `TocEntry`, `TrackType`
- `detect_disc_type()` — zero-allocation case-insensitive media type classification
- `open_tray()`, `close_tray()`, `drive_status()` — optical drive ioctl wrappers
- `read_toc()` — reads CD TOC via `CDROMREADTOCHDR`/`CDROMREADTOCENTRY`, computes track lengths and durations (75 frames/sec)
- `udev` module: `UdevEvent` parsing, `classify_device()`, `extract_capabilities()`, `classify_and_extract()` (single-pass)
- `device_info_from_udev()` — builds `DeviceInfo` from udev properties
- `enumerate_devices()` — walks sysfs, builds `DeviceInfo` for disks and partitions
- `UdevMonitor` — netlink socket (`AF_NETLINK`/`NETLINK_KOBJECT_UEVENT`), `poll()`, `run_with_listener()`, `subscribe()` channel API
- `parse_uevent()` — kernel uevent message parser (null-separated key=value)
- `linux` module: `LinuxDeviceManager` implementing `Device` trait
- `LinuxDeviceManager::mount()`, `unmount()`, `eject()` by device ID with cache updates
- `LinuxDeviceManager::start_monitor()` / `stop_monitor()` — background hotplug monitoring
- `LinuxDeviceManager::dispatch_event()` — listener dispatch with class-based filtering
- `error` module: `YantraError` with 11 variants, errno-to-error mapping helpers
- `tracing` instrumentation on all I/O operations (mount, unmount, eject, tray, monitor, enumerate)
- Feature gates: `udev`, `storage`, `optical` (all require `libc`), `ai` (requires `reqwest`, `tokio`)
- Criterion benchmarks: 45 benchmarks across 9 groups with 3-point history tracking
- `scripts/bench-history.sh` — runs benchmarks, appends to CSV, generates `BENCHMARKS.md`
- `scripts/version-bump.sh` — bumps VERSION and Cargo.toml
- GitHub Actions CI: fmt, clippy, test, bench, doc, cargo-deny
- GitHub Actions release: tag-triggered publish to crates.io + GitHub Release
- `Makefile` with standard targets (check, fmt, clippy, test, bench, audit, deny, coverage, doc)
- `deny.toml` — license, advisory, ban, and source checks
- `examples/detect.rs` — device detection, filesystem parsing, disc type detection
- 157 tests (12 hardware tests `#[ignore]`d), clippy clean with `-D warnings`

[Unreleased]: https://github.com/MacCracken/yantra/compare/v0.22.3...HEAD
[0.22.3]: https://github.com/MacCracken/yantra/releases/tag/v0.22.3
