# Testing Guide

## Running Tests

```bash
# Default features (udev, storage, optical)
cargo test

# All features including ai
cargo test --all-features

# Skip hardware tests (always skipped by default via #[ignore])
cargo test

# Run hardware tests (requires root + physical devices)
cargo test -- --ignored
```

## Test Categories

| Category | Count | Location |
|----------|-------|----------|
| Unit tests | ~130 | Colocated in each module |
| Hardware tests | ~12 | `#[ignore]` in storage, optical, udev, linux modules |
| Concurrent tests | ~1 | `event::tests::test_event_collector_concurrent` |

### Module Breakdown

| Module | Tests | Notes |
|--------|-------|-------|
| `device` | 15 | Bitflags, serde roundtrip, display, capabilities |
| `event` | 11 | Event creation, collector, concurrent access, serde |
| `storage` | 30 | Filesystem parsing, mount flags, mount point validation, /proc/mounts parsing, unescape |
| `optical` | 29 | Disc detection, TOC, tray state serde, ioctl constants, classify_toc_tracks |
| `udev` | 46 | Classification, capabilities, uevent parsing, sysfs enumeration, monitor lifecycle |
| `linux` | 8 | Manager lifecycle, listener dispatch, enumeration |
| `error` | 12 | All 11 error variants + Result type alias |

## Hardware Tests

Tests marked `#[ignore]` require physical hardware and/or root:

| Test | Requires |
|------|----------|
| `test_open_tray_hardware` | Optical drive (`/dev/sr0`) |
| `test_close_tray_hardware` | Optical drive |
| `test_drive_status_hardware` | Optical drive |
| `test_read_toc_hardware` | Optical drive with disc |
| `test_mount_hardware` | Block device + root |
| `test_unmount_hardware` | Mounted device + root |
| `test_eject_optical_hardware` | Optical drive + root |
| `test_eject_usb_hardware` | USB device + root |
| `test_monitor_create` | `CAP_NET_ADMIN` or root |
| `test_monitor_poll_timeout` | `CAP_NET_ADMIN` or root |
| `test_monitor_subscribe_stop` | `CAP_NET_ADMIN` or root |
| `test_find_real_mount` | Running system with mounts |

## Coverage

Target: 90%+ line coverage (see `codecov.yml`).

```bash
# Generate coverage report
cargo tarpaulin --all-features --skip-clean

# HTML report
cargo tarpaulin --all-features --out html
```

## Benchmarks

```bash
# Run benchmarks with history tracking
./scripts/bench-history.sh

# Just criterion (no history)
cargo bench --bench yukti_bench

# Via Makefile
make bench
```

Results are tracked in:
- `bench-history.csv` — rolling CSV of all runs
- `BENCHMARKS.md` — auto-generated with latest 3 runs for trend comparison

### Benchmark Groups

| Group | Benchmarks | What it measures |
|-------|-----------|-----------------|
| `device_id` | 2 | String allocation, Display |
| `device_info` | 9 | Construction, Cow returns, bitflag checks |
| `capabilities` | 4 | Bitflag operations (from_slice, to_vec, contains, OR) |
| `device_event` | 3 | Event creation, matches! checks |
| `event_collector` | 1 | Mutex throughput (100 events) |
| `serialization` | 4 | serde_json serialize/deserialize for DeviceInfo and DeviceEvent |
| `storage` | 8 | Filesystem parsing, mount point validation |
| `optical` | 6 | Disc type detection, TOC queries |
| `udev` | 8 | Device classification, capability extraction, full DeviceInfo construction |

## Testing Patterns

### Mock Mount Table

`find_mount_in()` takes a string so tests can inject mock `/proc/mounts` data:

```rust
let table = "/dev/sda1 / ext4 rw 0 0\n/dev/sdb1 /mnt/usb vfat rw 0 0\n";
let result = find_mount_in(Path::new("/dev/sdb1"), table);
assert_eq!(result, Some(PathBuf::from("/mnt/usb")));
```

### Mock Sysfs

`enumerate_devices()` and `LinuxDeviceManager::with_sysfs_root()` accept a custom path:

```rust
let dir = tempdir().unwrap();
// Create fake sysfs structure...
std::fs::create_dir_all(dir.path().join("block"));
let mgr = LinuxDeviceManager::with_sysfs_root(dir.path().to_path_buf());
let devices = mgr.enumerate().unwrap();
```

### Synthetic UdevEvent

Build events manually for classification testing:

```rust
let mut props = HashMap::new();
props.insert("ID_BUS".into(), "usb".into());
let event = UdevEvent {
    action: "add".into(),
    sys_path: PathBuf::from("/sys/devices/usb/block/sdb"),
    dev_path: Some(PathBuf::from("/dev/sdb")),
    subsystem: "block".into(),
    dev_type: Some("disk".into()),
    properties: props,
};
assert_eq!(classify_device(&event), DeviceClass::UsbStorage);
```

## Local CI

```bash
make check   # fmt + clippy + test + audit
```
