# cc5_aarch64 Codegen Bug — Native SIGILL on real aarch64

**Status**: **resolved in Cyrius 5.4.8** (2026-04-19). `cc5_aarch64`
sha `5d9e42cba3cdb430d2a376cadafa149c9ec8602ee770e2ff3ef9cb2927c4be74`
no longer emits the unallocated `0x800000d6` word. Verified on a
Raspberry Pi 4 (Cortex-A72) via `scripts/retest-aarch64.sh pi`:
`core_smoke`, all three fuzz targets, and the main `yukti` CLI run
to exit 0 on real aarch64 hardware.

**Remaining aarch64 blocker**: `yukti-test-aarch64` segfaults at
`test_query_permissions_dev_null` because yukti is compiled with
x86_64 Linux syscall numbers (syscall 4 = `stat` on x86_64, but
`pivot_root` on aarch64; dozens more mismatches across the codebase
and Cyrius stdlib `syscalls.cyr`). That is a yukti/stdlib portability
issue, not a toolchain bug — tracked separately in
[2026-04-19-aarch64-syscall-portability.md](2026-04-19-aarch64-syscall-portability.md).

**Retained as**: reproduction record for the historical opcode bug.
`scripts/retest-aarch64.sh pi` stays useful as a regression guard
against future codegen regressions.

---

## Original report (pre-fix, Cyrius 5.4.6 / 5.4.7-WIP)

## Reproduction

### Environment

| Layer            | Version / id                                                          |
|------------------|-----------------------------------------------------------------------|
| Build host       | x86_64 Linux (Arch), cyrius 5.4.6                                     |
| `cc5_aarch64`    | `sha256:32459694d9c5f3c4b764e696c7bac6c371fe6d4068cae0400bd605b48310636a` (local build from `~/Repos/cyrius` @ 5.4.7-WIP `7ef7b1c`) |
| Run host         | Raspberry Pi 4, Cortex-A72 (ARM implementer 0x41, part 0xd08)         |
| Kernel           | Linux 6.8.0-1051-raspi aarch64                                        |
| OS               | Ubuntu 24.04.4 LTS                                                    |
| CPU features     | fp asimd evtstrm crc32 cpuid (ARMv8-A baseline; no SVE, no dotprod)   |

### Steps

```sh
# 1. Install cc5_aarch64 (not shipped in the 5.4.6 release tarball).
cp ~/Repos/cyrius/build/cc5_aarch64 ~/.cyrius/bin/

# 2. Cross-build any yukti target. The minimal one is smallest:
cyrius build --aarch64 programs/core_smoke.cyr build/core_smoke-aarch64

# 3. Transfer to a real aarch64 host and run it.
scp build/core_smoke-aarch64 runner@agnosarm.local:/tmp/
ssh runner@agnosarm.local /tmp/core_smoke-aarch64
# → Illegal instruction (core dumped)
# → exit 132 (SIGILL, 128 + 4)
```

### Observed signal

`strace -i -v /tmp/core_smoke-aarch64` on the Pi:

```
[0000ffff94c2de0c] execve("./core_smoke-aarch64", [...]) = 0
[0000000000438394] --- SIGILL {si_signo=SIGILL, si_code=ILL_ILLOPC, si_addr=0x438394} ---
[????????????????] +++ killed by SIGILL (core dumped) +++
```

`si_code = ILL_ILLOPC` → illegal opcode (not a privileged or
alignment fault — the encoding itself is unallocated).

### Faulting instruction

ELF load base is `0x400000`, faulting PC is `0x438394`, so the
instruction lives at file offset `0x38394`. Raw bytes around
that point (`core_smoke-aarch64`, as cross-built by 5.4.6 today):

```
00038380: 54e8 1a80 d201 0000 d4bf 0300 91fd 7bc1
00038390: a8c0 035f d600 0080 d281 0296 d261 08a0
         ^^^^^^^^^^^^^^^^^^^^
         faulting instruction at +0x38394 = 0xd6 0x00 0x00 0x80
         little-endian word = 0x800000d6
```

Decoded by GNU binutils (`aarch64-linux-gnu-objdump` 2.46):

```
0:    800000d6    .inst    0x800000d6 ; undefined
```

Bit layout of `0x800000d6` (MSB → LSB):

```
10000000 00000000 00000000 11010110
^   ^^^^
|   op1 (bits 28:25) = 0b0000
op0 (bit 31) = 1
```

Per Arm ARM D17.2 (top-level encoding), `op1 == 0b00x0` is
**reserved / unallocated** in the ARMv8-A (and ARMv9-A) base ISA.
No CPU in current production decodes this as a valid instruction —
the faulting PC is a genuinely malformed encoding emitted by
`cc5_aarch64`, not a feature-gate issue.

### Affected targets

Tested 2026-04-19 on `runner@agnosarm.local`:

| Binary                         | Exit | Notes                           |
|--------------------------------|------|---------------------------------|
| `core_smoke-aarch64`           | 132  | SIGILL at PC 0x438394           |
| `yukti-test-aarch64` (tcyr)    | 132  | same class                      |
| `fuzz_mount_table-aarch64`     | 132  | same                            |
| `fuzz_parse_uevent-aarch64`    | 132  | same                            |
| `fuzz_partition_table-aarch64` | 132  | same                            |
| `yukti-aarch64` (main CLI)     | 132  | same                            |

Since `core_smoke-aarch64` has **zero syscalls and zero stdlib
imports** (it includes only `src/core.cyr` + `src/pci.cyr`), this
rules out:

- Syscall number translation (the compiler's x86→aarch64 syscall
  table is a sibling hypothesis; `core_smoke` doesn't call any).
- Yukti's own source — the failure reproduces with the smallest
  possible yukti target.
- Feature-gated instructions (Cortex-A72 is ARMv8-A baseline with
  the standard NEON/ASIMD set; the faulting opcode is unallocated
  for ALL ARMv8).

The bug is therefore in the `cc5_aarch64` code emitter. Likely
candidates: (a) a codegen path that reuses an x86-only opcode
template without re-encoding for aarch64, or (b) an entry-point
/ prologue emitter writing a constant word to the `.text`
section.

## What's already in place for when the fix lands

- **`scripts/retest-aarch64.sh`** — one command that cross-builds
  every yukti target, SCPs to a test host, runs each, reports
  pass/fail. First invocation to return all-zero is the signal
  that the toolchain is fixed.
- **CI hook** (`.github/workflows/ci.yml`) — `Cross-build
  aarch64` step. Currently gated on `cc5_aarch64` existing; when
  a toolchain release bundles it and the codegen fix, the step
  starts running automatically.
- **Release hook** (`.github/workflows/release.yml`) — produces
  `yukti-<ver>-aarch64-linux` artifact when `cc5_aarch64` is
  present. Will start shipping an aarch64 binary with the next
  tag after the fix.

## Upstream tracking

- Cyrius repo: `~/Repos/cyrius` (5.4.7-WIP at time of writing).
- Compiler binary sha: `32459694d9c5f3c4b764e696c7bac6c371fe6d4068cae0400bd605b48310636a`.
- To reproduce with a later compiler: replace `~/.cyrius/bin/cc5_aarch64`
  with the new build and run `./scripts/retest-aarch64.sh`.
- yukti's own source has no aarch64-specific branches — the
  entire fix, when it comes, will be toolchain-only.
