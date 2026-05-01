# Cyrius Toolchain Usage — Yukti

How to build, test, bundle, and release Yukti with the Cyrius toolchain.
This page is the single source of truth for commands; `CLAUDE.md` links
here instead of duplicating examples.

**Toolchain pin**: 5.7.48 (`cyrius = "5.7.48"` in `cyrius.cyml`).
`cyrius` provides `cc5` internally — never shell out to `cc5` directly.

Upgrade notes (5.5.11 → 5.7.48): the arc is mostly stdlib expansion
(json pretty-print/streaming/pointer in 5.7.40-5.7.42, sandhi
HTTP/TLS folded into stdlib at 5.7.0, Landlock + getrandom syscall
wrappers in 5.7.35) and aarch64 backend hardening (f64 basic ops
in v5.7.30, EB() codebuf cap raised in v5.7.34). Two latent
language gotchas surface during the bump — neither requires a
yukti code change but both are worth knowing:

- `var buf[N]` inside a function body is **static data**, not
  stack. Consecutive calls share the backing memory, so any
  `Str` or pointer that aliases into the buffer dangles on the
  next call. Yukti's parsing-bound buffers
  (`udev.cyr:666 parse_uevent`, `udev_rules.cyr:246 query_device`,
  `udev_rules.cyr:293 list_devices`) are safe because they pass
  through `str_from_buf` (`alloc + memcpy`) before any `Str`
  escapes; the syscall-buffer sites (`device.cyr:153
  query_permissions`, `partition.cyr:358 _parse_gpt_entries`,
  `storage.cyr:125 filesystem_usage`) only do scalar i64 loads.
  Build warning to watch for: "large static data (N bytes)".
- 5.x stdlib lookup helpers (`toml_get`, `args_get`, etc.) take
  cstr keys, not `Str` — passing `str_from("…")` silently returns
  0 because `str_eq_cstr` calls `strlen` on a NUL-less Str. Yukti
  uses `map_*` (cstr-keyed via `map_new()`) with bare-cstr literals
  or `str_cstr(s)` everywhere; no consumers of the affected helpers.

sandhi (HTTP/TLS service-boundary stdlib) is now available via
`lib/sandhi.cyr`; not pulled into yukti's `[deps] stdlib` because
yukti has no HTTP surface. Notable additions yukti doesn't
currently exercise but worth flagging:
`cyrius smoke` / `cyrius soak` subcommands (v5.7.38) — natural
fit for `programs/core_smoke.cyr`; `cyrius api-surface`
(v5.7.33) — public-API diff gate for downstream consumers;
`lib/security.cyr` Landlock + `lib/random.cyr` getrandom
(v5.7.35) — useful for path-traversal hardening.

aarch64 portability — 2.1.3 migrated 30
`SYS_OPEN`/`SYS_CLOSE`/`SYS_UNLINK` callers to stdlib wrappers;
2.1.4 finished the job with another 33 sites covering the
arch-divergent `read`/`write`/`stat`/`exit`/`mkdir`/`rmdir`/
`mount`/`umount2`/`lseek`/`socket`/`connect`/`statfs`/`newfstatat`/
`clock_gettime`/`ppoll` syscalls, and switched
`udev_monitor_poll` from poll(2) to ppoll(2) (aarch64 has no
SYS_POLL). Constants the stdlib doesn't expose live in
`src/syscalls.cyr` under arch-conditional `enum YkSyscalls`
blocks, plus a yukti-local `sys_stat` shim that fills the x86
gap (stdlib only ships `sys_stat` on aarch64). aarch64
cross-build is clean and runtime-correct; the only remaining
held aarch64 thread is the hardware-bound 5.4.6 SIGILL retest
on real Cortex-A72 (see
`docs/development/issues/2026-04-19-cc5-aarch64-repro.md`).

## Dependencies

Resolved by `cyrius deps` into `lib/` (gitignored; symlinks into
`~/.cyrius/deps/…`). Do **not** re-vendor them by hand.

- **Stdlib modules** (ship with Cyrius 5.7.48):
  `syscalls`, `string`, `alloc`, `str`, `fmt`, `vec`, `hashmap`, `io`,
  `fs`, `tagged`, `process`, `fnptr`, `chrono`, `args`, `freelist`
- **First-party deps** (pinned in `[deps.*]`):
  - `sakshi` 2.0.0 — structured logging
  - `patra` 1.9.2 — persistent device history

```sh
cyrius deps              # resolve [deps] into lib/
cyrius deps --lock       # write cyrius.lock (SHA256 of every lib/*.cyr)
cyrius deps --verify     # CI gate: fail on hash mismatch
```

## Build

```sh
cyrius build src/main.cyr build/yukti     # userland CLI (~384 KB static ELF)
```

Zero warnings is the gate. `dead:` lines from DCE are informational —
they confirm the reachable set is smaller than the linked set.

**aarch64 cross-build** (`cyrius build --aarch64 …`) compiles
cleanly to an aarch64 ELF, but binaries produced by Cyrius 5.4.6's
`cc5_aarch64` crashed with `SIGILL` on real hardware due to a
compiler codegen bug. Held pending retest on 5.7.48's `cc5_aarch64`
— the 5.5.x → 5.7.x arc lands real aarch64 fixes (EW alignment
assert v5.4.19, Apple Silicon Mach-O probe v5.5.11, f64 basic
ops v5.7.30, EB() codebuf cap raised v5.7.34); the original
Cortex-A72 repro has not yet been re-run.
See `docs/development/issues/2026-04-19-cc5-aarch64-repro.md` and
`scripts/retest-aarch64.sh`. The CI aarch64 gate is wired but
skips when `cc5_aarch64` isn't bundled with the toolchain
install, so current workflows pass.

## Test / Bench / Fuzz

```sh
cyrius test  tests/tcyr/yukti.tcyr        # 594 assertions, must be 0 failures
cyrius bench tests/bcyr/yukti.bcyr        # 45+ benchmarks (batch timing)
cyrius build fuzz/fuzz_parse_uevent.fcyr build/fuzz_parse_uevent
    ./build/fuzz_parse_uevent
cyrius build fuzz/fuzz_mount_table.fcyr  build/fuzz_mount_table
    ./build/fuzz_mount_table
```

Never claim a performance improvement without before/after benchmark
numbers. The CSV history in `docs/benchmarks/` is the proof.

## Dist Bundles (multi-profile, Cyrius 5.4.6+, current pin 5.7.48)

`cyrius distlib` concatenates `[lib] modules` (or `[lib.PROFILE]`) into
a single self-contained `.cyr` file, stripping `include` directives so
downstream consumers don't need Yukti's include chain.

```sh
cyrius distlib            # → dist/yukti.cyr       (full userland, ~5k lines)
cyrius distlib core       # → dist/yukti-core.cyr  (kernel-safe, ~450 lines)
```

Profiles are declared in `cyrius.cyml`:

```cyml
[lib]                      # default profile — full userland
modules = [ "src/error.cyr", "src/core.cyr", ... ]

[lib.core]                 # kernel-safe subset
modules = [ "src/core.cyr", "src/pci.cyr" ]
```

**Kernel-safe invariant**: `dist/yukti-core.cyr` must contain zero
`alloc`, `sys_*`, or `syscall` references and must link with no stdlib.
The invariant is enforced by `programs/core_smoke.cyr` — compile and
run it whenever `core.cyr` or `pci.cyr` changes:

```sh
cyrius build programs/core_smoke.cyr build/core_smoke && ./build/core_smoke
```

## Quality Gates

```sh
cyrius fmt <file> --check            # emits formatted output (diff vs source to enforce)
cyrius lint <file>                   # static checks; treat warnings as errors
cyrius vet src/main.cyr              # audit include dependencies
cyrius audit                         # full check: self-host, test, fmt, lint
```

`fmt --check` prints the formatted source — it does not diff. Pipe to
`diff` against the source to fail CI on a mismatch:

```sh
diff -q <(cyrius fmt src/main.cyr --check) src/main.cyr
```

## Release

```sh
cyrius publish            # tag + distlib + deps --lock + prints gh release command
```

`cyrius publish` is hands-off for git — it prints the `gh release
create` command but does not execute it. Push the tag and cut the
release yourself.

## Cyrius Language Conventions (Yukti-relevant subset)

- `var buf[N]` — N is **bytes**, not elements.
- `str_split(s, byte)` — separator is a byte value (10 for `\n`,
  32 for space).
- `str_contains_cstr(s, "needle")` — Str + cstr comparison.
- `str_index_of(s, byte)` — single-byte search (64 for `@`, 61 for `=`).
- `file_read_all(path, &buf, maxlen)` — 3 args; returns bytes read.
- `run(cmd, arg1, arg2)` — 3 args; returns `Result`.
- `dir_list(str_obj)` / `is_dir(str_obj)` — take `Str`, not cstr.
- No `mod` directives — flat namespace across the whole project.
- No closures capturing variables — benchmark callbacks are named
  `fn _b_*()` globals.
- All struct fields are 8 bytes (i64); access via `load64`/`store64`
  with named offset constants (`DH_TEMP`, `DI_LABEL`, ...).
- Enums for constants — zero `gvar_toks` cost.
- `str_builder` for formatting — avoid temporary allocations.
- Bump allocator (`alloc`) for long-lived heap data; freelist for
  data with individual lifetimes.
- Direct syscalls: `mount(165)`, `umount2(166)`, `ioctl(16)`,
  `socket(41)`. Arity warnings are errors.

## Never

- Shell out to `cc5` — always go through `cyrius <subcommand>`.
- Re-vendor stdlib or first-party deps into `src/` — let `cyrius deps`
  manage `lib/`.
- Add stdlib includes inside individual domain modules — `src/lib.cyr`
  owns the include chain.
- Claim a performance win without before/after benchmark numbers.
- Skip the `core_smoke` run after touching `core.cyr` / `pci.cyr` —
  the kernel-safe invariant is the whole point of the split.
