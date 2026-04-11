# Testing Guide

## Running Tests

```sh
# Build and run (407 assertions)
cat tests/yukti.tcyr | cc3 > build/yukti_test && chmod +x build/yukti_test
./build/yukti_test
```

Expected output:
```
=== error ===
=== device ===
=== event ===
=== storage ===
=== optical ===
=== udev ===
=== udev_rules ===
=== linux ===

407 passed, 0 failed (407 total)
```

## Test Coverage by Module

| Module | Test Functions | Key Coverage |
|--------|---------------|--------------|
| error (19) | All 16 error kinds, formatting, errno mapping, Result type |
| device (22) | All types, 10 capabilities, display name priority, size display, USB IDs, permissions, JSON |
| event (9) | All 6 event kinds, collector bulk, listener dispatch display |
| storage (18) | All 17 filesystem types, 9 forbidden mount points, octal unescape, mount table parsing |
| optical (12) | All 10 disc types, TOC operations, ioctl constants, tray state |
| udev (25) | All 8 device classes, capability extraction, uevent parsing, event conversion |
| udev_rules (7) | Rule rendering, 3 validation failures, accessors |
| linux (5) | Manager lifecycle, cache lookup, refresh |

## Running Benchmarks

```sh
cat benches/bench.bcyr | cc3 > build/yukti_bench && chmod +x build/yukti_bench
./build/yukti_bench
```

45 benchmarks using `bench_run_batch()` for nanosecond precision.

## Running Fuzz Targets

```sh
# Uevent parser — 1000 mutations + full truncation sweep
cat fuzz/fuzz_parse_uevent.fcyr | cc3 > build/fuzz_uevent && ./build/fuzz_uevent

# Mount table parser — 500 mutations + full truncation sweep
cat fuzz/fuzz_mount_table.fcyr | cc3 > build/fuzz_mount && ./build/fuzz_mount
```

## Writing Tests

```cyrius
include "src/lib.cyr"
include "lib/assert.cyr"

fn test_my_feature() {
    test_group("my_module");
    assert(condition, "description");
    assert_eq(actual, expected, "description");
    return 0;
}

fn main() {
    alloc_init();
    test_my_feature();
    var r = assert_summary();
    syscall(60, r);
}
main();
```

## Writing Benchmarks

Use named function pointers (no closures):

```cyrius
include "src/lib.cyr"
include "lib/bench.cyr"

fn _b_my_op() { my_function(); return 0; }

fn main() {
    alloc_init();
    var b = bench_new("module/operation");
    bench_run_batch(b, &_b_my_op, 10000, 100);
    bench_report(b);
    syscall(60, 0);
}
main();
```

## Benchmark History

```sh
./scripts/bench-history.sh
```

Appends to `bench-history.csv`, generates `BENCHMARKS.md` with 3-point trend tracking.

## Testing Patterns

### Mock Mount Table
`find_mount_in()` takes a Str so tests inject mock data:
```cyrius
var table = str_from("/dev/sda1 / ext4 rw 0 0\n/dev/sdb1 /mnt/usb vfat rw 0 0\n");
var mp = find_mount_in("/dev/sdb1", table);
assert(str_eq_cstr(mp, "/mnt/usb"), "found");
```

### Synthetic UdevEvent
```cyrius
var props = map_new();
map_set(props, "ID_BUS", str_from("usb"));
var e = udev_event_new(str_from("add"), str_from("/sys/block/sdb"),
    str_from("/dev/sdb"), str_from("block"), str_from("disk"), props);
assert_eq(classify_device(e), DC_USB_STORAGE, "usb classified");
```

### Mock Sysfs
```cyrius
var mgr = linux_dm_with_root(str_from("/tmp/fake_sysfs"));
var r = linux_dm_enumerate(mgr);
```
