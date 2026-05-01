# Yukti — Claude Code Instructions

## Project Identity

**Yukti** (Sanskrit: device/instrument) — Device abstraction for AGNOS:
USB storage, optical drives, block devices, GPU, network filesystems,
udev hotplug, mount/eject.

- **Type**: Flat library (include-based) + multi-profile dist bundles
- **License**: GPL-3.0-only
- **Language**: Cyrius (sovereign systems language, compiled by cc5)
- **Version**: SemVer, version file at `VERSION`
- **Status**: 2.1.0 — shipping as `lib/yukti.cyr` in Cyrius stdlib since 3.4.12
- **Genesis repo**: [agnosticos](https://github.com/MacCracken/agnosticos)
- **Standards**: [First-Party Standards](https://github.com/MacCracken/agnosticos/blob/main/docs/development/applications/first-party-standards.md)
- **Shared crates**: [shared-crates.md](https://github.com/MacCracken/agnosticos/blob/main/docs/development/applications/shared-crates.md)

## Goal

Own device abstraction. One library answers "what hardware is on this
box, and what can I do with it?" across USB, optical, block, GPU, and
network devices. Kernel-safe subset (`core.cyr` + `pci.cyr`) compiles
without alloc or syscalls so AGNOS itself can identify PCI devices
using the same tables userland uses.

## Scaffolding

Ported from Rust (April 2026). Structure follows first-party AGNOS
conventions: `src/lib.cyr` include chain, `tests/tcyr/`, `tests/bcyr/`,
`fuzz/`, `programs/`, `dist/`. Do not restructure manually — match
conventions so downstream projects can read this one without
re-learning the layout.

## Current State

- **Source**: ~5920 lines across 17 domain modules (`src/*.cyr`)
- **Tests**: 639 assertions, 3 fuzz harnesses, 45+ benchmarks
- **Binary**: ~384 KB x86_64 static ELF, zero external dependencies
- **Stable**: 2.2.0 — audio device discovery via new `src/audio.cyr` (enumerates ALSA PCM devices over `/dev/snd/` + `/proc/asound/` with PCI-BDF / USB-VID:PID anchored hw_ids; surfaces the typed descriptor adapter API for vani 0.3.x's `vani_open_yukti(desc, direction)`). `DC_AUDIO = 9` appended to DeviceClass. Fixed long-standing `_parse_uevent_key` bug in gpu.cyr (was returning whole uevent text instead of value). 2.1.4 — aarch64 *runtime* correct (33 raw-number `syscall(N, …)` sites migrated to stdlib wrappers / `SYS_*` constants; new `src/syscalls.cyr` adds arch-conditional definitions for socket-family + statfs / newfstatat / clock_gettime / ppoll where stdlib has gaps; `udev_monitor_poll` switched poll→ppoll for arch portability). 2.1.3 — aarch64 cross-build clean (30 SYS_OPEN/SYS_CLOSE/SYS_UNLINK sites migrated to stdlib wrappers; patra dep bumped 1.1.1 → 1.9.2 with the matching migration). Kernel-safe subset, multi-profile dist, P(-1) security audit closed (all HIGH/MED/LOW fixed), dual-layer / dual-sided disc support, audio CD ripping API, fuzzed parsers (uevent, mount table, partition table).
- **Toolchain**: Cyrius 5.7.48 (`cyrius.cyml: cyrius = "5.7.48"`)
- **Integration**: consumed by jalwa, aethersafha, argonaut, the AGNOS
  file manager; kernel-safe subset consumed by AGNOS kernel

## Consumers

| Project      | Usage                                              |
|--------------|----------------------------------------------------|
| jalwa        | Auto-import on USB attach                          |
| file manager | Device sidebar (USB, optical, block, network)      |
| aethersafha  | Mount/unmount notifications                        |
| argonaut     | Automount of removable media                       |
| AGNOS kernel | `dist/yukti-core.cyr` — PCI class/vendor tables    |

## Dependencies

- **Cyrius stdlib** — `syscalls`, `string`, `alloc`, `str`, `fmt`, `vec`,
  `hashmap`, `io`, `fs`, `tagged`, `process`, `fnptr`, `chrono`,
  `args`, `freelist` (ships with Cyrius >= 5.7.48)
- **sakshi** 2.0.0 — structured logging (first-party)
- **patra** 1.9.2 — persistent device history (first-party)

No external deps. No FFI. No libc. All first-party, pinned in
`cyrius.cyml` and SHA-locked in `cyrius.lock`.

## Quick Start

See [`docs/development/cyrius-usage.md`](docs/development/cyrius-usage.md)
for the full command reference: build, test, bench, fuzz, distlib
(multi-profile), deps lock/verify, and release.

At a glance:

```bash
cyrius deps                              # resolve deps into lib/
cyrius build src/main.cyr build/yukti    # build CLI
cyrius test tests/tcyr/yukti.tcyr        # 639 assertions
cyrius distlib                           # → dist/yukti.cyr (full)
cyrius distlib core                      # → dist/yukti-core.cyr (kernel-safe)
```

## Architecture

```
src/
  lib.cyr          — include chain (deps + domain modules, in order)
  main.cyr         — CLI entry point (device enumeration)
  error.cyr        — 16 error kinds, heap-allocated error structs
  core.cyr         — kernel-safe enums, struct layouts, accessors
  pci.cyr          — kernel-safe PCI class/vendor tables + predicates
  device.cyr       — userland constructors, serializers, sysfs queries
  event.cyr        — DeviceEvent, EventCollector, listener dispatch
  storage.cyr      — Filesystem enum, mount/unmount/eject, /proc/mounts
  optical.cyr      — DiscType, tray control, TOC reading via ioctls
  udev.cyr         — UdevEvent, sysfs enumeration, netlink monitor
  linux.cyr        — LinuxDeviceManager (ties modules together)
  udev_rules.cyr   — rule rendering, validation, udevadm wrappers
  partition.cyr    — MBR + GPT table reading
  device_db.cyr    — persistent device history via patra
  network.cyr      — SMB/NFS mount helpers
  gpu.cyr          — GPU probe via sysfs
  audio.cyr        — ALSA PCM enumeration + vani descriptor adapter
programs/
  core_smoke.cyr   — kernel-safe invariant check (core + pci only)
dist/
  yukti.cyr        — full userland bundle (`cyrius distlib`)
  yukti-core.cyr   — kernel-safe bundle (`cyrius distlib core`)
tests/tcyr/        — 639 assertions across all modules
tests/bcyr/        — benchmarks with batch timing
fuzz/              — 2 fuzz targets (uevent parser, mount table parser)
docs/benchmarks/   — auto-generated results.md + history.csv
cyrius.cyml        — package manifest (toolchain pin, [deps], [lib.*] profiles)
cyrius.lock        — SHA256 lockfile for every lib/*.cyr dep
```

**Include order matters.** `src/lib.cyr` declares the full chain: stdlib
first, first-party deps, then domain modules in dependency order.
Stdlib includes live **only** in `lib.cyr` — never in individual
domain modules. Domain modules are flat: zero transitive includes,
which is what makes `cyrius distlib` (strip-include concatenation)
produce a compile-clean bundle.

## Key Constraints

- **Kernel-safe subset is sacred** — `core.cyr` + `pci.cyr` must have
  zero alloc, zero syscalls, zero stdlib dependencies. The
  `programs/core_smoke.cyr` smoke test is the tripwire.
- **All values are i64 or fixed-size strings** — matches Cyrius type system.
- **No floating point** — integer math only.
- **Manual struct layout** — `alloc()` + `load64`/`store64` with named
  offset constants (`DI_LABEL`, `DH_TEMP`, ...). No anonymous offsets.
- **Enums for constants** — zero `gvar_toks` cost vs. `var` globals.
- **str_builder for formatting** — avoid temporary allocations.
- **Bump allocator for long-lived data**; freelist for data with
  individual lifetimes (e.g. event collectors).
- **sakshi logging on all device operations** — structured observability
  across attach/detach/mount/eject.
- **Direct syscalls** — `mount(165)`, `umount2(166)`, `ioctl(16)`,
  `socket(41)`. No libc wrappers.

## Development Process

### P(-1): Scaffold Hardening (before any new features)

0. Read roadmap, CHANGELOG, open issues — know what was intended
1. Cleanliness: `cyrius build` (0 warnings), `cyrius lint` (0 warnings),
   `cyrius fmt --check` diff-clean, `cyrius vet src/main.cyr` clean
2. Test sweep: 531+ assertions pass, fuzz harnesses pass
3. Benchmark baseline: `cyrius bench tests/bcyr/yukti.bcyr`, save CSV
4. Internal deep review — gaps, optimizations, correctness, docs
5. External research — udev / sysfs / block-layer changes since last pass
6. Security audit (see below) — file findings in `docs/audit/YYYY-MM-DD-audit.md`
7. Additional tests / benchmarks from findings
8. Post-review benchmarks — prove the wins
9. Documentation audit — CLAUDE.md, roadmap, CHANGELOG, cyrius-usage.md
10. Repeat if heavy

### Work Loop (continuous)

1. Work phase — new features, roadmap items, bug fixes
2. Build check: `cyrius build src/main.cyr build/yukti` — 0 warnings
3. Test + benchmark additions for new code
4. Internal review — performance, memory, correctness
5. **If `core.cyr` or `pci.cyr` changed**: rebuild and run `core_smoke`
6. Security check — any new syscall usage, user input handling, buffer
   allocation reviewed for safety
7. Documentation — CHANGELOG, roadmap, docs
8. Version check — `VERSION`, `cyrius.cyml`, CHANGELOG header in sync
9. Return to step 1

### Security Hardening (before release)

1. **Input validation** — every function accepting external data
   (`/proc/mounts`, uevent strings, partition tables, sysfs) validates
   bounds, types, ranges before use
2. **Buffer safety** — every `var buf[N]` and `alloc(N)` verified:
   N in bytes, max offset < N, no adjacent-variable overflow
3. **Syscall review** — every `syscall()` / `sys_*()` reviewed: args
   validated, return values checked, error paths handled
4. **Pointer validation** — no raw deref of untrusted input without
   bounds check
5. **No command injection** — no `sys_system()` / `exec_cmd()` with
   unsanitized input. Use `exec_vec()` with explicit argv
6. **No path traversal** — mount-point paths validated against allowed
   directories; no `../` escape
7. **Known CVE check** — review against current udev / kernel block-layer
   CVEs
8. **File findings** — `docs/audit/YYYY-MM-DD-audit.md` with severity,
   file, line, fix

Severity levels: **CRITICAL** (exploitable immediately) / **HIGH**
(moderate effort) / **MEDIUM** (specific conditions) / **LOW**
(defense-in-depth).

### Closeout Pass (before every minor/major bump)

Ship as the last patch of the current minor (e.g. 1.2.5 before 1.3.0):

1. Full test suite — 531+ pass, 0 failures
2. Benchmark baseline — `cyrius bench`, save CSV for comparison
3. Dead code audit — review `dead:` list from `cyrius build`, remove
   unreferenced source
4. Stale comment sweep — grep for old version refs, outdated TODOs,
   stale "pending Cyrius X.Y.Z" comments
5. Security re-scan — grep for new `sys_system`, unchecked writes,
   unsanitized input, buffer size mismatches
6. Downstream check — jalwa, aethersafha, argonaut, AGNOS kernel still
   build and pass tests against new version
7. CHANGELOG / roadmap sync — docs reflect current state; version
   numbers consistent across `VERSION`, `cyrius.cyml`, CHANGELOG header,
   intended git tag
8. Kernel-safe invariant — `core_smoke` passes; `dist/yukti-core.cyr`
   contains zero `alloc` / `sys_*` / `syscall` references
9. Full build from clean — `rm -rf build lib && cyrius deps &&
   cyrius build` passes clean; both dist bundles regenerate clean

### Task Sizing

- **Low/Medium effort**: batch freely — multiple items per work loop cycle
- **Large effort**: small bites only — break into sub-tasks, verify each
- **If unsure**: treat it as large

### Refactoring Policy

- Refactor when the code tells you to — duplication, unclear
  boundaries, measured bottlenecks
- Never refactor speculatively. Wait for the third instance
- Every refactor passes the same test + fuzz + benchmark gates as new code
- 3 failed attempts = defer and document

## Key Principles

- **Correctness is the optimum sovereignty** — if it's wrong, you don't
  own it, the bugs own you
- **Numbers don't lie** — never claim a performance improvement without
  before/after benchmark numbers. The CSV history is the proof
- **Own the stack** — zero external dependencies; direct syscalls
- **No magic** — every operation measurable, auditable, traceable
- Test after EVERY change, not after the feature is done
- ONE change at a time — never bundle unrelated changes
- Fuzz every parser path — uevent, mount table, partition tables
- Programs must call `main()` at top level:
  `var exit_code = main(); syscall(60, exit_code);`
- `cyrius build` handles everything — NEVER use raw `cat file | cc5`
- Source files only need project includes — deps auto-resolve from
  `cyrius.cyml`
- Every buffer declaration is a contract: `var buf[N]` = N bytes

## Cyrius Conventions

The full list of Yukti-relevant Cyrius idioms (buffer semantics,
`str_split` byte separators, `run()` arity, flat namespace rules, etc.)
lives in [`docs/development/cyrius-usage.md`](docs/development/cyrius-usage.md).
Read it before writing a module — avoiding the common traps
(`var buf[N]` is bytes and is static data when declared inside a
function, no closures over locals, `break` in `var`-heavy loops
unreliable) saves a lot of debug time.

## CI / Release

- **Toolchain pin**: `cyrius = "5.7.48"` in `cyrius.cyml`. Release and CI
  both read from the manifest; no hardcoded versions in YAML
- **Dead code elimination**: `cyrius build` already strips unreachable
  functions; the `dead:` report is informational
- **Tag filter**: release workflow triggers on `tags: ['[0-9]*']` — semver only
- **Version-verify gate**: release asserts `VERSION == cyrius.cyml version ==
  git tag` before building
- **Lint gate**: CI runs `cyrius lint` per source; treat warnings as errors
- **Lock gate**: CI runs `cyrius deps --verify` against committed
  `cyrius.lock`; mismatch fails the build
- **Dist gate**: CI regenerates `dist/yukti.cyr` and `dist/yukti-core.cyr`
  via `cyrius distlib` and `cyrius distlib core`; both must compile-check
  clean
- **Kernel-safe gate**: CI builds and runs `programs/core_smoke.cyr` —
  non-zero exit fails the build
- **Concurrency**: CI uses `cancel-in-progress: true` keyed on workflow + ref

## Key References

- [`docs/development/cyrius-usage.md`](docs/development/cyrius-usage.md)
  — toolchain commands, distlib profiles, lint/fmt gates
- [`docs/architecture/overview.md`](docs/architecture/overview.md)
  — module map, data flow, struct layouts
- [`docs/development/roadmap.md`](docs/development/roadmap.md)
  — milestones, backlog, future
- [`docs/development/threat-model.md`](docs/development/threat-model.md)
  — attack surface, mitigations
- [`docs/benchmarks/results.md`](docs/benchmarks/results.md)
  — latest numbers
- [`docs/benchmarks/history.csv`](docs/benchmarks/history.csv)
  — regression baseline
- `CHANGELOG.md` — source of truth for all changes

## DO NOT

- **Do not commit or push** — the user handles all git operations
- **NEVER use `gh` CLI** — use `curl` to GitHub API only
- Do not add external dependencies — first-party only
- Do not skip benchmarks before claiming performance improvements
- Do not skip fuzz verification before claiming a parser works
- Do not use `mod` directives (causes namespace prefixing issues)
- Do not add Cyrius stdlib includes in individual `src/*.cyr` files —
  `src/lib.cyr` owns the whole include chain
- Do not use `sys_system()` with unsanitized input — command injection risk
- Do not add alloc / syscall usage to `core.cyr` or `pci.cyr` — breaks
  the kernel-safe invariant
- Do not re-vendor stdlib or first-party deps into `src/` — `cyrius
  deps` manages `lib/`
- Do not hardcode toolchain versions in CI YAML — read `cyrius.cyml`
- Do not shell out to `cc5` directly — always go through `cyrius <subcommand>`
