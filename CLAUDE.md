# Yukti — Claude Code Instructions

## Project Identity

**Yukti** (Sanskrit: device/instrument) — Device abstraction — USB, optical, block devices, udev hotplug, mount/eject

- **Language**: Cyrius 5.2.x (ported from Rust, April 2026)
- **Type**: Flat library (include-based) + single-file dist bundle
- **License**: GPL-3.0-only
- **Version**: see VERSION (`${file:VERSION}` in manifest)
- **Binary**: ~360 KB static ELF, zero external dependencies
- **Manifest**: `cyrius.cyml` — stdlib + sakshi + patra via `[deps]`
- **Shipped as**: `lib/yukti.cyr` in the Cyrius stdlib (since 3.4.12)

## Consumers

jalwa (auto-import), file manager (device sidebar), aethersafha (mount notifications), argonaut (automount)

## Build & Test

Requires the `cyrius` toolchain 5.2.x on PATH (which provides `cc5`
internally). Deps are resolved into `lib/` by `cyrius deps`; the stdlib
modules (alloc, str, fmt, vec, hashmap, io, fs, tagged, json, process,
fnptr, chrono, args, freelist) and external deps (sakshi 2.0.0, patra
1.1.1) come through that mechanism — do NOT re-vendor them by hand.

```sh
# Resolve deps into lib/ (once, and after any dep change)
cyrius deps

# Build the CLI
cyrius build src/main.cyr build/yukti

# Test (485 assertions)
cyrius test tests/tcyr/yukti.tcyr

# Benchmark
cyrius bench tests/bcyr/yukti.bcyr

# Fuzz
cyrius build fuzz/fuzz_parse_uevent.fcyr build/fuzz_parse_uevent && ./build/fuzz_parse_uevent
cyrius build fuzz/fuzz_mount_table.fcyr  build/fuzz_mount_table  && ./build/fuzz_mount_table

# Bundle for stdlib distribution
cyrius distlib              # writes dist/yukti.cyr

# Supply-chain integrity
cyrius deps --lock          # write cyrius.lock (SHA256 of every lib/*.cyr dep)
cyrius deps --verify        # CI gate: fail on hash mismatch

# Release
cyrius publish              # tag + distlib + lock + print gh release command
```

No Makefile — the `cyrius` tool is the whole build system. Never shell
out to `cc5` directly; always go through `cyrius <subcommand>`.

## Project Structure

```
src/
  lib.cyr              — Include chain (entry point for library consumers)
  main.cyr             — CLI entry point (device enumeration)
  error.cyr            — 16 error kinds, heap-allocated error structs
  device.cyr           — DeviceId, DeviceInfo, DeviceClass, DeviceCapabilities
  event.cyr            — DeviceEvent, EventCollector, listener dispatch
  storage.cyr          — Filesystem enum, mount/unmount/eject, /proc/mounts
  optical.cyr          — DiscType, tray control, TOC reading via ioctls
  udev.cyr             — UdevEvent, sysfs enumeration, netlink monitor
  linux.cyr            — LinuxDeviceManager (ties modules together)
  udev_rules.cyr       — Rule rendering, validation, udevadm wrappers
  partition.cyr        — MBR + GPT table reading
  device_db.cyr        — Persistent device history via patra
  network.cyr          — Network filesystem mount helpers (SMB/NFS)
  gpu.cyr              — GPU probe via sysfs
lib/                   — Dep dir, managed by `cyrius deps` (gitignored;
                         symlinks into ~/.cyrius/deps/…); do NOT edit
tests/
  tcyr/yukti.tcyr      — 485 assertions across all modules
  bcyr/yukti.bcyr      — Benchmarks with batch timing
fuzz/                  — 2 fuzz targets (uevent parser, mount table parser)
dist/yukti.cyr         — Single-file bundle produced by `cyrius distlib`
docs/benchmarks/       — Auto-generated results.md + history.csv +
                         rust-v-cyrius.md
cyrius.cyml            — Package manifest (replaces old cyrius.toml)
cyrius.lock            — SHA256 lockfile for every lib/*.cyr dep
```

## Development Process

### Development Loop (continuous)

1. Work phase — new features, roadmap items, bug fixes
2. Build: `cyrius build src/main.cyr build/yukti` — must be 0 warnings
3. Test: `cyrius test tests/tcyr/yukti.tcyr` — must be 0 failures
4. Benchmark additions for new code
5. Run benchmarks (`./scripts/bench-history.sh`)
6. Audit phase — review performance, memory, security, correctness
7. Fuzz new parsers
8. Bundle: `cyrius distlib` — verify dist/yukti.cyr rebuilds cleanly
9. Lock: `cyrius deps --lock` — only when a dep version moves
10. Documentation — update CHANGELOG, roadmap, docs
11. Return to step 1

### Key Principles

- **Never skip benchmarks.** Numbers don't lie. The CSV history is the proof.
- **Tests + benchmarks are the way.** 485+ assertions, 45+ benchmarks.
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
