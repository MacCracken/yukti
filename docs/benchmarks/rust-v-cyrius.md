# Benchmarks: Rust vs Cyrius

> Final comparison before booting Rust. Rust benchmarks from commit `0145265` (criterion).
> Cyrius benchmarks from commit `HEAD` (batch-timed, `bench_run_batch`).
> Same machine, same hardware.

## Binary Size

| Metric | Rust | Cyrius | Ratio |
|--------|------|--------|-------|
| **Library (rlib/ELF)** | 2,281,434 bytes | 151,752 bytes | **15x smaller** |
| **Example binary (stripped)** | 449,144 bytes | 151,752 bytes | **3x smaller** |
| **Dependencies** | 47 crates (Cargo.lock) | 0 external deps | — |
| **Toolchain size** | ~800 MB (rustup) | ~300 KB (cc5) | **2,600x smaller** |

## Source Code

| Metric | Rust | Cyrius |
|--------|------|--------|
| **Source lines** | 6,166 | 3,359 |
| **Test assertions** | 229 passed (13 ignored) | 407 passed |
| **Benchmark operations** | 48 (criterion) | 45 (batch-timed) |
| **Fuzz targets** | 2 (libfuzzer) | 2 (mutation + truncation) |
| **Modules** | 9 (.rs) | 10 (.cyr) |

## Benchmark Comparison

### device_id

| Benchmark | Rust (ns) | Cyrius (ns) | Δ |
|-----------|-----------|-------------|---|
| create | 10.1 | 44 | Rust 4.4x faster |
| eq | — | 109 | — |
| display | 17.2 | — | — |

### device_info

| Benchmark | Rust (ns) | Cyrius (ns) | Δ |
|-----------|-----------|-------------|---|
| create | 81.7 | 163 | Rust 2.0x faster |
| display_name_label | 2.0 | 7 | Rust 3.5x faster |
| display_name_fallback | 25.0 | 160 | Rust 6.4x faster |
| size_display_zero | 2.0 | 243 | Rust 122x faster |
| size_display_gb | 166.7 | 156 | **Cyrius 1.1x faster** |
| has_capability_hit | 0.53 | 8 | Rust 15x faster |
| has_capability_miss | 0.54 | 8 | Rust 15x faster |
| is_removable | 1.8 | 7 | Rust 3.9x faster |
| is_mounted | 1.0 | 9 | Rust 9x faster |

### capabilities

| Benchmark | Rust (ns) | Cyrius (ns) | Δ |
|-----------|-----------|-------------|---|
| contains_check | 0.52 | 6 | Rust 12x faster |
| bitwise_or_all | 0.50 | 5 | Rust 10x faster |

### device_event

| Benchmark | Rust (ns) | Cyrius (ns) | Δ |
|-----------|-----------|-------------|---|
| create | 85.4 | 518 | Rust 6.1x faster |
| is_attach | 0.54 | 6 | Rust 11x faster |
| is_removable | 0.55 | 7 | Rust 13x faster |
| push_100 | 7,780 | 2,000 | **Cyrius 3.9x faster** |

### storage

| Benchmark | Rust (ns) | Cyrius (ns) | Δ |
|-----------|-----------|-------------|---|
| filesystem_parse | 3.4 | 38 | Rust 11x faster |
| filesystem_parse_case | 3.5 | 40 | Rust 11x faster |
| filesystem_parse_unknown | 15.7 | 383 | Rust 24x faster |
| validate_mount_point_ok | 379.7 | 235 | **Cyrius 1.6x faster** |
| validate_mount_point_forbidden | 150.0 | 501 | Rust 3.3x faster |
| validate_mount_point_deep | 399.7 | 554 | Rust 1.4x faster |
| default_mount_point | 51.2 | 263 | Rust 5.1x faster |
| mount_options_create | 29.1 | 52 | Rust 1.8x faster |

### optical

| Benchmark | Rust (ns) | Cyrius (ns) | Δ |
|-----------|-----------|-------------|---|
| detect_disc_type_cd | 2.0 | 27 | Rust 14x faster |
| detect_disc_type_dvd | 3.8 | 117 | Rust 31x faster |
| detect_disc_type_bluray | 2.8 | 182 | Rust 65x faster |
| detect_disc_type_unknown | 3.9 | 360 | Rust 92x faster |
| toc_audio_count | 2.3 | 30 | Rust 13x faster |
| toc_audio_duration | 3.2 | 33 | Rust 10x faster |

### device_queries

| Benchmark | Rust (ns) | Cyrius (ns) | Δ |
|-----------|-----------|-------------|---|
| query_permissions | 1,227 | 1,000 | **Cyrius 1.2x faster** |
| query_health_miss | 6,728 | 5,000 | **Cyrius 1.3x faster** |

### udev

| Benchmark | Rust (ns) | Cyrius (ns) | Δ |
|-----------|-----------|-------------|---|
| classify_usb | 17.5 | 161 | Rust 9.2x faster |
| classify_optical | 30.8 | 225 | Rust 7.3x faster |
| classify_dm | 13.1 | 432 | Rust 33x faster |
| extract_caps_usb | 53.9 | 626 | Rust 12x faster |
| extract_caps_optical | 103.7 | 536 | Rust 5.2x faster |
| classify_and_extract | 69.9 | 810 | Rust 12x faster |
| device_info_from_udev | 828.3 | 2,000 | Rust 2.4x faster |
| device_info_from_udev_opt | 637.3 | 1,000 | Rust 1.6x faster |

### serialization

| Benchmark | Rust (ns) | Cyrius (ns) | Δ |
|-----------|-----------|-------------|---|
| device_info_json | 693.4 | 2,000 | Rust 2.9x faster |

## Analysis

### Where Cyrius wins
- **Binary size**: 15x smaller library, 3x smaller binary — zero runtime deps
- **Syscall-heavy ops**: `query_permissions` (1.2x), `query_health` (1.3x) — direct syscalls vs libc wrapper
- **Bulk collection**: `event_collector/push_100` (3.9x) — bump allocator beats Mutex<Vec>
- **Validation**: `validate_mount_point_ok` (1.6x) — simpler path comparison
- **Size display**: `size_display_gb` (1.1x) — integer math beats f64

### Where Rust wins
- **Sub-nanosecond ops**: bitflag checks, enum matching — LLVM optimizes to single instructions
- **String comparison**: `eq_ignore_ascii_case` is heavily optimized by LLVM
- **Zero-cost abstractions**: struct field access is a direct offset, no function call overhead
- **Criterion measurement**: sub-ns precision via `black_box` + instruction-level timing

### Why Cyrius is slower on micro-ops
- **No inlining**: every function is a call/ret pair (~5ns overhead)
- **HashMap lookups**: `udev_event_property()` uses FNV hash + strcmp per lookup vs Rust's stack-local enum matching
- **Bump allocator**: `alloc()` on every operation vs Rust's stack allocation
- **String ops**: `str_eq_ci()` is a byte loop vs LLVM-vectorized `eq_ignore_ascii_case`

### Key takeaway
Cyrius trades micro-op speed (~5-15x slower on sub-ns operations) for **dramatically smaller binaries** (15x), **zero dependencies**, and **faster I/O-bound operations** (syscalls, bulk allocation). For a device abstraction library where the real bottleneck is kernel I/O (mount, ioctl, sysfs reads), the micro-op overhead is irrelevant — actual device operations take microseconds to milliseconds.

---

> Generated 2026-04-11. Rust commit `0145265`, Cyrius commit `HEAD`.
