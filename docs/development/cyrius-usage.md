# Cyrius Toolchain Usage — Yukti

How to build, test, bundle, and release Yukti with the Cyrius toolchain.
This page is the single source of truth for commands; `CLAUDE.md` links
here instead of duplicating examples.

**Toolchain pin**: 5.5.11 (`cyrius = "5.5.11"` in `cyrius.cyml`).
`cyrius` provides `cc5` internally — never shell out to `cc5` directly.

Upgrade notes (5.4.8 → 5.5.11): the intervening 5.4.9–5.5.11 arc is
entirely Windows PE / Apple Silicon Mach-O / aarch64 backend work —
no language-level breaking changes for Linux x86_64. Clean bump,
no source changes required. Notable behavioral additions yukti
doesn't currently exercise: `--strict` CLI flag (v5.4.19) escalates
undef-fn warnings to hard errors; `#ifplat PLAT` / `#endplat`
directives (v5.4.19) as a cleaner alternative to
`#ifdef CYRIUS_ARCH_*`; fncall arity ceiling raised 6→8 (v5.4.13).

## Dependencies

Resolved by `cyrius deps` into `lib/` (gitignored; symlinks into
`~/.cyrius/deps/…`). Do **not** re-vendor them by hand.

- **Stdlib modules** (ship with Cyrius 5.5.11):
  `syscalls`, `string`, `alloc`, `str`, `fmt`, `vec`, `hashmap`, `io`,
  `fs`, `tagged`, `json`, `process`, `fnptr`, `chrono`, `args`,
  `freelist`
- **First-party deps** (pinned in `[deps.*]`):
  - `sakshi` 2.0.0 — structured logging
  - `patra` 1.1.1 — persistent device history

```sh
cyrius deps              # resolve [deps] into lib/
cyrius deps --lock       # write cyrius.lock (SHA256 of every lib/*.cyr)
cyrius deps --verify     # CI gate: fail on hash mismatch
```

## Build

```sh
cyrius build src/main.cyr build/yukti     # userland CLI (~362 KB static ELF)
```

Zero warnings is the gate. `dead:` lines from DCE are informational —
they confirm the reachable set is smaller than the linked set.

**aarch64 cross-build** (`cyrius build --aarch64 …`) compiles
cleanly to an aarch64 ELF, but binaries produced by Cyrius 5.4.6's
`cc5_aarch64` crashed with `SIGILL` on real hardware due to a
compiler codegen bug. Held pending retest on 5.5.11's `cc5_aarch64`
— the Cyrius 5.5.x arc includes aarch64 backend touches
(EW alignment assert in v5.4.19, Apple Silicon Mach-O probe in
v5.5.11); the original Cortex-A72 repro has not yet been re-run.
See `docs/development/issues/2026-04-19-cc5-aarch64-repro.md` and
`scripts/retest-aarch64.sh`. The CI aarch64 gate is wired but
skips when `cc5_aarch64` isn't bundled with the toolchain
install, so current workflows pass.

## Test / Bench / Fuzz

```sh
cyrius test  tests/tcyr/yukti.tcyr        # 592 assertions, must be 0 failures
cyrius bench tests/bcyr/yukti.bcyr        # 45+ benchmarks (batch timing)
cyrius build fuzz/fuzz_parse_uevent.fcyr build/fuzz_parse_uevent
    ./build/fuzz_parse_uevent
cyrius build fuzz/fuzz_mount_table.fcyr  build/fuzz_mount_table
    ./build/fuzz_mount_table
```

Never claim a performance improvement without before/after benchmark
numbers. The CSV history in `docs/benchmarks/` is the proof.

## Dist Bundles (multi-profile, Cyrius 5.4.6+, current pin 5.5.11)

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
