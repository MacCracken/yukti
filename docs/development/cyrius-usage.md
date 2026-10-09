# Cyrius Toolchain Usage — Yukti

How to build, test, bundle, and release Yukti with the Cyrius toolchain.
This page is the single source of truth for commands; `CLAUDE.md` links
here instead of duplicating examples.

**Toolchain pin**: 6.7.5 (`cyrius = "6.7.5"` in `cyrius.cyml`).
`cyrius` drives the compiler internally (`cycc`, named `cc5` before
Cyrius 6.0) — never shell out to it directly.

Upgrade notes — **historical record** (5.5.11 → 5.7.48, a full major
behind the current 6.7.5 pin; retained for the two language gotchas,
not as current guidance): that arc was mostly stdlib expansion
(json pretty-print/streaming/pointer in 5.7.40-5.7.42, sandhi
HTTP/TLS folded into stdlib at 5.7.0, Landlock + getrandom syscall
wrappers in 5.7.35) and aarch64 backend hardening (f64 basic ops
in v5.7.30, EB() codebuf cap raised in v5.7.34). Two latent
language gotchas surfaced during the bump — neither required a
yukti code change but both still hold:

- `var buf[N]` inside a function body is **static data**, not
  stack. Consecutive calls share the backing memory, so any
  `Str` or pointer that aliases into the buffer dangles on the
  next call. Yukti's parsing-bound buffers
  (`udev.cyr:683 udev_monitor_poll`, `udev_rules.cyr:246 query_device`,
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
SYS_POLL). 2.1.4 left the numbers the stdlib did not yet name
in arch-conditional `enum YkSyscalls` blocks in `src/syscalls.cyr`.

2.3.12 (cyrius 6.6.6) retired them:
- Every kernel call now goes through a stdlib wrapper (`sys_*`,
  or the portable `x*` family in `lib/io.cyr`).
- One of those local numbers was wrong. The aarch64
  `SYS_STATFS = 43` was renumbered by ESYSXLAT into accept(2),
  so `filesystem_usage` had never worked on ARM.
- `src/syscalls.cyr` now holds only the `_yk_*` bridges and one
  x86_64 number, ppoll's 271. The bridges cover the agnos ABI
  splits: mount, umount2, statfs, lstat, the socket family and
  ioctl.
- `_yk_ppoll` is the single raw `syscall()` in the tree, because
  no stdlib peer wraps ppoll.

(The yukti-local `sys_stat` shim was dropped at the 6.0.1 bump;
the stdlib ships `sys_stat` on x86_64 too now.) The aarch64
cross-build is clean, and the tcyr suite passes 845/845 under
`qemu-aarch64`. The remaining held aarch64 thread is
hardware-bound — the full-target retest on real Cortex-A72 has
not been re-run since the 2.1.4 migration (see
`docs/development/issues/2026-04-19-aarch64-syscall-portability.md`).

## Dependencies

Resolved by `cyrius deps` into `lib/` (gitignored; symlinks into
`~/.cyrius/deps/…`). Do **not** re-vendor them by hand.

- **Stdlib modules** (ship with Cyrius 6.7.5):
  `syscalls`, `string`, `alloc`, `str`, `fmt`, `vec`, `hashmap`, `io`,
  `fs`, `tagged`, `process`, `fnptr`, `chrono`, `args`, `freelist`,
  `atomic`, `sync`, `thread_local`
- **First-party deps** (pinned in `[deps.*]`):
  - `sakshi` 2.5.8 — structured logging
  - `patra` 1.16.0 — persistent device history

  Normally both tags stay in lockstep with the versions the pinned
  cyrius bundles. 2.3.16 pins the W2 stdlib-wave tags, which are AHEAD
  of the 6.7.5 fold (sakshi 2.5.7, patra 1.15.2) until cyrius 6.7.6
  refolds them. Until then `cyrius build` warns that `./lib/` shadows
  the pinned patra, and `lib/sakshi.cyr` is the bundled 2.5.7 (patra's
  sidecar names sakshi as a stdlib leaf, and the fold wins) while
  `cyrius.lock` records the 2.5.8 commit. `cyrius.cyml` has the details.

```sh
cyrius deps              # resolve [deps] into lib/
cyrius deps --lock       # write cyrius.lock (SHA256 of every lib/*.cyr)
cyrius deps --verify     # CI gate: fail on hash mismatch
```

## Build

```sh
cyrius build src/main.cyr build/yukti     # userland CLI (~477 KB static ELF; ~157 KB with CYRIUS_DCE=1)
```

Zero warnings is the gate. `dead:` lines from DCE are informational —
they confirm the reachable set is smaller than the linked set.

**aarch64 cross-build** (`cyrius build --aarch64 …`) compiles
cleanly to an aarch64 ELF. The historical `SIGILL` codegen bug in
Cyrius 5.4.6's `cc5_aarch64` was fixed upstream in 5.4.8 and
verified on real Cortex-A72 for `core_smoke`, the three fuzz
targets, and the main CLI — but the full-target retest, the tcyr
suite in particular, has not been re-run on hardware since the
2.1.4 syscall migration, so aarch64 stays **held** pending a
retest under the current 6.6.6 toolchain. The backend binary was
renamed `cc5_aarch64` → `cycc_aarch64` in Cyrius 6.0.
See `docs/development/issues/2026-04-19-aarch64-syscall-portability.md`,
`docs/development/issues/archive/2026-04-19-cc5-aarch64-repro.md`, and
`scripts/retest-aarch64.sh` (which accepts either backend name).
The CI aarch64 gate is required, not optional — it hard-fails when
neither `cycc_aarch64` nor `cc5_aarch64` is present in the
toolchain install.

## Test / Bench / Fuzz

```sh
cyrius test  tests/tcyr/yukti.tcyr        # 845 assertions, must be 0 failures
cyrius bench tests/bcyr/yukti.bcyr        # 46 benchmarks (batch timing)
cyrius build fuzz/fuzz_parse_uevent.fcyr    build/fuzz_parse_uevent
    ./build/fuzz_parse_uevent
cyrius build fuzz/fuzz_mount_table.fcyr     build/fuzz_mount_table
    ./build/fuzz_mount_table
cyrius build fuzz/fuzz_partition_table.fcyr build/fuzz_partition_table
    ./build/fuzz_partition_table
```

Never claim a performance improvement without before/after benchmark
numbers. The CSV history in `docs/benchmarks/` is the proof.

## Dist Bundles (multi-profile, Cyrius 5.4.6+, current pin 6.7.5)

`cyrius distlib` concatenates `[lib] modules` (or `[lib.PROFILE]`) into
a single self-contained `.cyr` file, stripping `include` directives so
downstream consumers don't need Yukti's include chain.

```sh
cyrius distlib            # → dist/yukti.cyr       (full userland, ~7.0k lines)
cyrius distlib core       # → dist/yukti-core.cyr  (kernel-safe, ~460 lines)
```

Profiles are declared in `cyrius.cyml`:

```cyml
[lib]                      # default profile — full userland
modules = [ "src/syscalls.cyr", "src/error.cyr", ... ]

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
cyrius fmt <file> --check            # drift check: diagnostic + exit 1, non-destructive
cyrius fmt <file> --dry              # same, plus a "WOULD reformat" summary line
cyrius fmt <file>                    # REWRITES THE FILE IN PLACE
cyrius lint <file>                   # static checks; treat warnings as errors
cyrius vet src/lib.cyr               # audit include deps (lib.cyr, not main.cyr: vet does not recurse)
cyrius audit                         # project sweep: fmt/lint/docs/tests/bench
```

⚠ **`cyrius fmt <file>` with no flag rewrites the file in place and prints
nothing.** It did once print the formatted source to stdout; since 6.5.29
it does not (re-verified on 6.6.6). Anything that captures its stdout gets an empty string.

Use `--check` for gates. It prints the offending file and its first
differing line, exits 1, and leaves the file untouched; on a clean file it
is silent and exits 0. Verified both directions during the 2.3.3 bump
(6.5.29) and again at 2.3.12 (6.6.6). `--dry` is `--check` plus a trailing summary line.

This corrects the pre-2.3.3 guidance here and in CI, which said
`--check` was "exit-code-only" and told you to diff the no-flag form's
stdout instead:

```sh
diff -q <(cyrius fmt src/main.cyr) src/main.cyr   # ⚠ BROKEN on 6.5.29
```

That compares empty output against the file, so it reports drift on every
file no matter how clean the tree is — and, because the no-flag form
*writes*, it silently reformats the checkout as a side effect. The
canonical continuation indent it enforces is **2 spaces per open paren**
(4 also accepted).

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
- Kernel calls go through stdlib wrappers (`sys_read`, `sys_ioctl`,
  `sys_statfs`, …), the portable `x*` helpers (`xopen`, `xmkdir`,
  `xlseek`, …), or a `_yk_*` bridge in `src/syscalls.cyr`. Never
  raw `syscall()`, whether the number is a literal or a `SYS_*`
  name: both hard-code one target's ABI. The sole exception is
  `_yk_ppoll`, and CI enforces it (2.3.12). Open flags are spelled
  as `O_*`, never numbers. Arity mismatches are hard errors since
  cyrius 6.5.1.

## Never

- Shell out to `cycc` (`cc5` pre-6.0) — always go through
  `cyrius <subcommand>`.
- Re-vendor stdlib or first-party deps into `src/` — let `cyrius deps`
  manage `lib/`.
- Add stdlib includes inside individual domain modules — `src/lib.cyr`
  owns the include chain.
- Claim a performance win without before/after benchmark numbers.
- Skip the `core_smoke` run after touching `core.cyr` / `pci.cyr` —
  the kernel-safe invariant is the whole point of the split.
