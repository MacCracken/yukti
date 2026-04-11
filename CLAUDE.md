# Yukti — Claude Code Instructions

## Project Identity

**Yukti** (Sanskrit: device/instrument) — Device abstraction — USB, optical, block devices, udev hotplug, mount/eject

- **Language**: Cyrius (ported from Rust, April 2026)
- **Type**: Flat library (include-based)
- **License**: GPL-3.0-only
- **Version**: 1.0.0
- **Binary**: 152 KB static ELF, zero external dependencies

## Consumers

jalwa (auto-import), file manager (device sidebar), aethersafha (mount notifications), argonaut (automount)

## Build & Test

```sh
# Build (requires cc3 compiler)
cat src/main.cyr | cc3 > build/yukti

# Test (407 assertions)
cat tests/yukti.tcyr | cc3 > build/yukti_test && ./build/yukti_test

# Benchmark (45 operations)
cat benches/bench.bcyr | cc3 > build/yukti_bench && ./build/yukti_bench

# Fuzz
cat fuzz/fuzz_parse_uevent.fcyr | cc3 > build/fuzz_uevent && ./build/fuzz_uevent
cat fuzz/fuzz_mount_table.fcyr | cc3 > build/fuzz_mount && ./build/fuzz_mount
```

## Project Structure

```
src/
  lib.cyr          — Include chain (entry point for library consumers)
  main.cyr         — CLI demo (device enumeration)
  error.cyr        — 16 error kinds, heap-allocated error structs
  device.cyr       — DeviceId, DeviceInfo, DeviceClass, DeviceCapabilities
  event.cyr        — DeviceEvent, EventCollector, listener dispatch
  storage.cyr      — Filesystem enum, mount/unmount/eject, /proc/mounts
  optical.cyr      — DiscType, tray control, TOC reading via ioctls
  udev.cyr         — UdevEvent, sysfs enumeration, netlink monitor
  linux.cyr        — LinuxDeviceManager (ties modules together)
  udev_rules.cyr   — Rule rendering, validation, udevadm wrappers
lib/               — Vendored Cyrius stdlib (24 modules)
tests/yukti.tcyr   — 407 assertions across all modules
benches/bench.bcyr — 45 benchmarks with batch timing
fuzz/              — 2 fuzz targets (uevent parser, mount table parser)
rust-old/          — Original Rust source (archived)
```

## Development Process

### Development Loop (continuous)

1. Work phase — new features, roadmap items, bug fixes
2. Build: `cat src/main.cyr | cc3 > build/yukti` — must be 0 warnings
3. Test: `cat tests/yukti.tcyr | cc3 > build/yukti_test && ./build/yukti_test` — must be 0 failures
4. Benchmark additions for new code
5. Run benchmarks (`./scripts/bench-history.sh`)
6. Audit phase — review performance, memory, security, correctness
7. Fuzz new parsers
8. Documentation — update CHANGELOG, roadmap, docs
9. Return to step 1

### Key Principles

- **Never skip benchmarks.** Numbers don't lie. The CSV history is the proof.
- **Tests + benchmarks are the way.** 407+ assertions, 45+ benchmarks.
- **Own the stack.** Zero external dependencies. Direct syscalls.
- **No magic.** Every operation is measurable, auditable, traceable.
- **Enums for constants** — zero gvar_toks cost.
- **Manual struct layout** — alloc + store64/load64 with offset constants.
- **Accessor functions** — `fn structname_field(ptr)` pattern.
- **str_builder for formatting** — avoid temporary allocations.
- **Bump allocator** — alloc() for all heap data.
- **sakshi logging** — structured logging on all operations.
- **Direct syscalls** — mount(165), umount2(166), ioctl(16), socket(41).

## Cyrius Conventions

- `var buf[N]` — N is bytes, not elements
- `str_split(s, byte)` — separator is a byte value (10 for \n, 32 for space)
- `str_contains_cstr(s, "needle")` — for Str + cstr comparison
- `str_index_of(s, byte)` — finds single byte (64 for @, 61 for =)
- `file_read_all(path, &buf, maxlen)` — 3 args, returns bytes read
- `run(cmd, arg1, arg2)` — 3 args, returns Result
- `dir_list(str_obj)` / `is_dir(str_obj)` — take Str, not cstr
- No `mod` directive — flat namespace
- No closures capturing variables — use named functions + globals for benchmarks

## DO NOT
- **Do not commit or push** — the user handles all git operations
- **NEVER use `gh` CLI** — use `curl` to GitHub API only
- Do not add unnecessary dependencies — keep it lean
- Do not skip benchmarks before claiming performance improvements
- Do not use `mod` directives (causes namespace prefixing issues)
