# Yukti — Claude Code Instructions

## Project Identity

**Yukti** (Sanskrit: device/instrument) — Device abstraction for AGNOS:
USB storage, optical drives, block devices, GPU, network filesystems,
udev hotplug, mount/eject.

- **Type**: Flat library (include-based) + multi-profile dist bundles
- **License**: GPL-3.0-only
- **Language**: Cyrius (sovereign systems language, compiled by cc5)
- **Version**: SemVer, version file at `VERSION`
- **Status**: 2.3.5 — shipping as `lib/yukti.cyr` in Cyrius stdlib since 3.4.12
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

- **Source**: 6,380 lines across 18 files in `src/` — 16 domain modules
  (6,246 lines, matching the `[lib]` module list in `cyrius.cyml`) plus
  the `lib.cyr` include chain and the `main.cyr` CLI entry point
- **Tests**: 753 assertions, 3 fuzz harnesses, 46 benchmarks
- **Binary**: ~457 KB x86_64 static ELF (467,784 bytes, `CYRIUS_DCE=1`),
  zero external dependencies
- **Stable**: 2.3.5 — repair release, no new public surface. `str_starts_with` takes a **Str** prefix and the stdlib has NO cstr peer for it, so all 6 yukti call sites passed a cstr and got UNDEFINED behaviour (measured: 0 for every input against a rodata literal, 1 for every input against `str_cstr()` output) — `list_devices` parsed nothing, no SMB share was ever detected, the `/run/media/` rmdir guard never fired, optical eject took the sysfs path; fixed with `_yk_starts_with_cstr` in src/syscalls.cyr. Partitions were NEVER enumerated on any machine: the partition loop's `continue` aborted it at the first non-matching sysfs entry (measured 1 of 5 iterations in situ) — trigger NOT isolated, six synthetic repros all behave correctly, so the other 27 `continue` sites were left alone rather than rewritten on a hypothesis. Also: every data track was classified as audio (cdte_ctrl is the HIGH nibble, so the mask is 0x40 not 0x04); `list_devices` read 8 KB of a 344,721-byte udev DB; `mount_count` never incremented; `network_probe_host` turned any hostname into 0.0.0.0 = localhost; `ppoll` rejected the conventional -1 timeout. The 8 mock-sysfs tests deferred since April found the two headline bugs — their "blocker" had been stale the whole time. 2.3.4 — P(-1) audit/refactor/hardening/security sweep (ten adversarially-verified dimensions; full write-up in `docs/audit/2026-08-19-audit.md`). No finding survived at HIGH. Fixed: sakshi spans leaked on 18 error paths — the 16-slot stack behind a never-reset global means 16 leaks permanently kill ALL structured logging, measured 0→4 across four error paths and 0→0 after; every `struct stat` offset in `src/` was an x86_64 literal, so `query_permissions` read st_uid as the mode on aarch64 (second instance of the storage.cyr TOCTOU-guard bug — `src/` had ZERO named `STAT_*`), now per-target constants with agnos failing closed since its stat has no uid/gid at all; `network_list_mounted` had re-introduced the 8 KB `/proc/mounts` truncation audit MED-3 fixed, because that fix was inline rather than shared — now a common `read_procfs_text` that reports truncation; udev rule names and rule content were both injectable (path traversal into `O_CREAT|O_TRUNC`, and `RUN+=` runs as root). The span leak was found by a manual deferral review, NOT by the sweep — see 'What the sweep did not catch'. 2.3.3 — toolchain + dependency refresh plus the one regression it carried: cyrius 6.5.3 → 6.5.29, sakshi 2.4.6 → 2.4.10, patra 1.12.12 → 1.13.8 (6.5.29 bundles exactly the latter two, clearing both the shadow-lib and toolchain-drift warnings). patra ≥1.13.6 REJECTS a >255-byte STR (`PATRA_ERR_ROWSZ`) where ≤1.12.12 truncated it silently, and yukti discarded all 12 `patra_exec` returns — so any device with a >255-byte identifier stopped being recorded at all, silently. Reachable: `_read_sysfs_attr` reads into a `var attr_buf[256]`, one byte over the cap, and audio's USB `hw_id` concatenates two such reads. Fixed by clamping in `_sql_escape_str` (the single funnel both stored values and WHERE literals pass through, so they still match — this also repairs `is_known`, broken under 1.12.12 too) and by routing the 8 mutating writes through a new `_db_exec` that warns on non-`PATRA_OK`. New boundary tests at 254/255/256/300 bytes, verified to produce 7 failures when the clamp is disabled. The bump broke CI's format gate — 6.5.29's `cyrius fmt <file>` no longer prints to stdout, it rewrites in place, so the gate's `diff <(cyrius fmt $f) $f` compared empty-vs-file and failed all 24 gated sources on a correctly-formatted tree; moved to `cyrius fmt $f --check`. 8 files reformatted to the current canonical continuation indent, proven whitespace-only (`git diff -w` empty) and semantically inert (byte-identical DCE binary). patra's public function set and error constants are unchanged 1.12.12 → 1.13.8; the deltas are behavioural (WHERE type mismatches now error, over-long STR now errors, WAL v3 → v4 with a database identity). 2.3.2 — raw syscall cleanup: 40 numeric `syscall(N, …)` sites outside `src/` migrated to stdlib wrappers. 20 raw `syscall(87)` unlinks were `timerfd_gettime` on aarch64, writing 32 bytes through an uninitialised register and corrupting the test counters — the aarch64 suite reported `182 passed, 0 failed` instead of 658, masking 2 real failures. CI now rejects raw numeric syscalls. 2.3.1 — completed the 2.3.0 agnos ABI sweep: `_yk_mkdir` (agnos
  `sys_mkdir` is `(path, pathlen)`, POSIX is `(path, mode)` — same arity, so no
  compiler diagnostic) and `_yk_umount2` (`sys_umount2` does not exist on agnos;
  fails closed rather than routing to the 0-arity `sys_umount` stub that returns
  success). Toolchain pin 6.5.2 → 6.5.3 with `cyrius.lock` regenerated — it had
  been frozen at 6.4.67-era stdlib, which was masking an undefined `xrmdir`.
  2.3.0 — six agnos syscall ABI mismatches, one of them (`sys_mount`) fabricating
  success. 2.2.1 — audio domain follow-on: `SUBSYSTEM=sound` events with `pcmC*D*` DEVPATH filter classify as `DC_AUDIO`; new `audio_devices` table + `device_db_record_audio_seen`/`_audio_known`/`_audio_last_seen`/`_audio_count` API key persistence by `hw_id` so re-plugging carries history forward. 2.2.0 — audio device discovery via new `src/audio.cyr` (enumerates ALSA PCM devices over `/dev/snd/` + `/proc/asound/` with PCI-BDF / USB-VID:PID anchored hw_ids; surfaces the typed descriptor adapter API for vani 0.3.x's `vani_open_yukti(desc, direction)`). `DC_AUDIO = 9` appended to DeviceClass. Fixed long-standing `_parse_uevent_key` bug in gpu.cyr (was returning whole uevent text instead of value). 2.1.4 — aarch64 *runtime* correct (33 raw-number `syscall(N, …)` sites migrated to stdlib wrappers / `SYS_*` constants; new `src/syscalls.cyr` adds arch-conditional definitions for socket-family + statfs / newfstatat / clock_gettime / ppoll where stdlib has gaps; `udev_monitor_poll` switched poll→ppoll for arch portability). 2.1.3 — aarch64 cross-build clean (30 SYS_OPEN/SYS_CLOSE/SYS_UNLINK sites migrated to stdlib wrappers; patra dep bumped 1.1.1 → 1.9.2 with the matching migration). Kernel-safe subset, multi-profile dist, P(-1) security audit closed (all HIGH/MED/LOW fixed), dual-layer / dual-sided disc support, audio CD ripping API, fuzzed parsers (uevent, mount table, partition table).
- **Toolchain**: Cyrius 6.5.29 (`cyrius.cyml: cyrius = "6.5.29"`)
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
  `args`, `freelist`, `atomic`, `sync`, `thread_local`
  (ships with Cyrius >= 6.5.29). The last three are patra 1.13.8's
  transitive requirements — cyrius 6.4.x+ requires them named explicitly
  in `[deps].stdlib`. As of 2.3.3 `cyrius distlib` also reports all three
  in `dist/yukti.deps` (18 leaves, was 15).
- **sakshi** 2.4.10 — structured logging (first-party)
- **patra** 1.13.8 — persistent device history (first-party)

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
cyrius test tests/tcyr/yukti.tcyr        # 658 assertions
cyrius distlib                           # → dist/yukti.cyr (full)
cyrius distlib core                      # → dist/yukti-core.cyr (kernel-safe)
```

## Architecture

```
src/
  lib.cyr          — include chain (deps + domain modules, in order)
  main.cyr         — CLI entry point (device enumeration)
  syscalls.cyr     — arch-conditional SYS_* constants + agnos ABI
                     bridges (_yk_mount, _yk_umount2, _yk_mkdir)
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
tests/tcyr/        — 658 assertions across all modules
tests/bcyr/        — benchmarks with batch timing
fuzz/              — 3 fuzz targets (uevent, mount table, partition table)
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
- **Direct syscalls** — `mount` / `umount2` through the `_yk_*` bridges
  in `src/syscalls.cyr`, `ioctl` / `socket` through arch-conditional
  `SYS_*` constants. No raw syscall numbers, no libc wrappers.

## Development Process

### P(-1): Scaffold Hardening (before any new features)

0. Read roadmap, CHANGELOG, open issues — know what was intended
1. Cleanliness: `cyrius build` (0 warnings), `cyrius lint` (0 warnings),
   `cyrius fmt --check` diff-clean, `cyrius vet src/main.cyr` clean
2. Test sweep: 658+ assertions pass, fuzz harnesses pass
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

1. Full test suite — 658+ pass, 0 failures
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

- **Toolchain pin**: `cyrius = "6.5.29"` in `cyrius.cyml`. Release and CI
  both read from the manifest; no hardcoded versions in YAML
- **Dead code elimination**: `cyrius build` already strips unreachable
  functions; the `dead:` report is informational
- **Tag filter**: release workflow triggers on `tags: ['[0-9]*']` — semver only
- **Version-verify gate**: release asserts `VERSION == cyrius.cyml version ==
  git tag` before building
- **Lint gate**: CI runs `cyrius lint` per source; treat warnings as errors
- **Format gate**: CI runs `cyrius fmt <file> --check` per source. Must be
  `--check` — the no-flag `cyrius fmt <file>` rewrites in place and prints
  nothing on 6.5.29, so diffing its stdout fails every file and mutates the
  checkout (fixed at 2.3.3)
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
