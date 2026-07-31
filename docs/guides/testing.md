# Testing Guide

## Running Tests

```sh
# Build and run (658 assertions)
cyrius test tests/tcyr/yukti.tcyr
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
=== device_db ===
=== partition ===
=== network ===
=== gpu ===
=== pci ===
=== audio direction constants ===
=== audio parse pcmC0D3p (basic playback) ===
... (15 audio groups: PCM parsing, classification, audio history)

658 passed, 0 failed (658 total)
```

## Test Coverage by Module

| Module | Test Functions | Key Coverage |
|--------|---------------|--------------|
| error (19) | All 16 error kinds, formatting, errno mapping, Result type |
| device (21) | All types, 10 capabilities, display name priority, size display, USB IDs, permissions, JSON |
| event (9) | All 6 event kinds, collector bulk, listener dispatch display |
| storage (24) | All 17 filesystem types, 16 forbidden mount points, octal unescape, mount table parsing |
| optical (17) | All 15 disc types, TOC operations, ioctl constants, tray state |
| udev (25) | All 10 device classes, capability extraction, uevent parsing, event conversion |
| udev_rules (6) | Rule rendering, 3 validation failures, accessors |
| linux (5) | Manager lifecycle, cache lookup, refresh |
| device_db (5) | Open/close lifecycle, record seen, preferences, mount count, known lookup |
| partition (6) | MBR + GPT constants, type strings, entry accessors, GUID helpers, real table read |
| network (5) | SMB/NFS types, share construction, mount source, mounted list, probe constants |
| gpu (5) | Vendor names, sysfs enumeration, count, report, device class |
| pci (9) | Kernel-safe class/vendor tables, class to device type, name fallback, predicates |
| audio (17) | PCM name parsing, direction constants, bit-pack roundtrip, hotplug classification, audio history table |

## Running Benchmarks

```sh
cyrius bench tests/bcyr/yukti.bcyr
```

46 benchmarks using `bench_run_batch()` for nanosecond precision.

## Running Fuzz Targets

```sh
# Uevent parser — 1000 mutations + full truncation sweep
cyrius build fuzz/fuzz_parse_uevent.fcyr build/fuzz_parse_uevent && ./build/fuzz_parse_uevent

# Mount table parser — 500 mutations + full truncation sweep
cyrius build fuzz/fuzz_mount_table.fcyr build/fuzz_mount_table && ./build/fuzz_mount_table

# Partition table parser (MBR + GPT) — 500 mutations + truncation sweep
cyrius build fuzz/fuzz_partition_table.fcyr build/fuzz_partition_table && ./build/fuzz_partition_table
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

Appends to `docs/benchmarks/history.csv`, regenerates `docs/benchmarks/results.md` with 3-point trend tracking.

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
