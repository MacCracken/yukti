# aarch64 Syscall Portability — x86_64 numbers hardcoded throughout

**Status**: open. With Cyrius 5.4.8's `cc5_aarch64` codegen fix, the
aarch64 cross-build now produces executable binaries (see
[2026-04-19-cc5-aarch64-repro.md](2026-04-19-cc5-aarch64-repro.md)),
but yukti's test binary segfaults on real aarch64 hardware because
every `syscall(...)` site in yukti — and the `SYS_*` enum yukti
inherits from the Cyrius stdlib — uses **x86_64 Linux** syscall
numbers. aarch64 Linux uses the generic syscall table
(`include/uapi/asm-generic/unistd.h`), which overlaps with x86_64
on almost nothing.

**Impact**: yukti ships x86_64-only. The cross-build path is
wired, `core_smoke` and the three fuzz harnesses pass on aarch64
(they make zero or trivial syscalls), but any userland code path
that touches the filesystem, block devices, mount table, netlink,
or sockets will either silently misbehave or segfault.

**Filed for**: v2.2.0 scope discussion. Not a 2.1.x patch —
fixing this properly needs a stdlib-level arch dispatch or per-arch
`SYS_*` constants, and every bare-literal `syscall(N, …)` site in
yukti has to be audited.

## Reproduction

### Environment

| Layer         | Version / id                                           |
|---------------|--------------------------------------------------------|
| Build host    | x86_64 Linux (Arch), Cyrius 5.4.8                      |
| `cc5_aarch64` | sha `5d9e42cba3cdb430d2a376cadafa149c9ec8602ee770e2ff3ef9cb2927c4be74` |
| Run host      | Raspberry Pi 4, Cortex-A72, Ubuntu 24.04.4 LTS         |
| Kernel        | Linux 6.8.0-1051-raspi aarch64                         |
| Yukti         | 2.1.0 @ `main` 287053c                                 |

### Steps

```sh
cyrius build --aarch64 tests/tcyr/yukti.tcyr build/yukti-test-aarch64
scp build/yukti-test-aarch64 pi:/tmp/
ssh pi /tmp/yukti-test-aarch64
# → FAIL: perms not null
# → Segmentation fault (core dumped)
# → exit 139 (SIGSEGV, 128 + 11)
```

The other five cross-built targets (`yukti`, `core_smoke`, three
fuzz harnesses) all exit 0, so the SIGILL opcode class of failure
is genuinely resolved. The tcyr suite is the first target that
exercises a substantive number of real syscalls.

### Faulting path

`tests/tcyr/yukti.tcyr:371-376`:

```
fn test_query_permissions_dev_null() {
    var perms = query_permissions("/dev/null");
    assert(perms != 0, "perms not null");
    assert_eq(perms_uid(perms), 0, "perms uid 0");
    return 0;
}
```

`src/device.cyr:152-166`:

```
pub fn query_permissions(dev_path_cstr) {
    var buf[144];
    var r = syscall(4, dev_path_cstr, &buf);    # ← x86_64 stat
    if (r < 0) { return 0; }

    var mode = load32(&buf + 24);               # ← x86_64 struct stat layout
    var uid  = load32(&buf + 28);
    var gid  = load32(&buf + 32);
    ...
}
```

On x86_64, syscall 4 is `stat(2)` and the returned `struct stat`
has `st_mode` at offset 24, `st_uid` at 28, `st_gid` at 32.

On aarch64 Linux, **syscall 4 is `pivot_root(2)`**. The kernel's
generic syscall table (which aarch64 uses verbatim) has no
`stat`/`lstat`/`fstat` at all — userland must use
`newfstatat(79)` or `fstat(80)`. Calling syscall 4 with a path
pointer and a buffer pointer either returns `-EINVAL` or, if the
running user has CAP_SYS_ADMIN, silently triggers `pivot_root`
semantics. In the observed failure the call returns a non-negative
value, `perms != 0` passes, and then `perms_uid` reads garbage
that eventually segfaults downstream.

## Scope of the portability gap

### Bare-literal syscall numbers in yukti source

Every number below is x86_64-specific and needs an aarch64 variant
(the aarch64 generic-table number is shown for reference):

| File            | Line(s)                                   | x86_64 | Syscall name        | aarch64 |
|-----------------|-------------------------------------------|--------|---------------------|---------|
| `main.cyr`      | 11, 12, 19, 60, 63, 80                    | 1      | `write`             | 64      |
| `main.cyr`      | 81, 87                                    | 60     | `exit`              | 93      |
| `device.cyr`    | 154                                       | 4      | `stat`              | — (use 79 `newfstatat`) |
| `event.cyr`     | 63                                        | 228    | `clock_gettime`     | 113     |
| `device_db.cyr` | 140, 209                                  | 228    | `clock_gettime`     | 113     |
| `storage.cyr`   | 126                                       | 137    | `statfs`            | 43      |
| `storage.cyr`   | 361                                       | 2      | `open`              | — (use 56 `openat`) |
| `storage.cyr`   | 368                                       | 0      | `read`              | 63      |
| `storage.cyr`   | 385                                       | 3      | `close`             | 57      |
| `storage.cyr`   | 557                                       | 83     | `mkdir`             | — (use 34 `mkdirat`) |
| `storage.cyr`   | 568                                       | 262    | `newfstatat`        | 79      |
| `storage.cyr`   | 596, 612                                  | 165    | `mount`             | 40      |
| `storage.cyr`   | 633                                       | 166    | `umount2`           | 39      |
| `storage.cyr`   | 656                                       | 84     | `rmdir`             | — (use 35 `unlinkat` + AT_REMOVEDIR) |
| `storage.cyr`   | 774                                       | 1      | `write`             | 64      |
| `udev_rules.cyr`| 155, 156                                  | 1      | `write`             | 64      |
| `udev_rules.cyr`| 175                                       | 87     | `unlink`            | — (use 35 `unlinkat`) |
| `network.cyr`   | 177                                       | 83     | `mkdir`             | — (use 34 `mkdirat`) |
| `network.cyr`   | 191                                       | 165    | `mount`             | 40      |
| `network.cyr`   | 300                                       | 41     | `socket`            | 198     |
| `network.cyr`   | 331                                       | 42     | `connect`           | 203     |
| `network.cyr`   | 332                                       | 3      | `close`             | 57      |
| `partition.cyr` | 161, 169                                  | 8      | `lseek`             | 62      |
| `partition.cyr` | 164, 171                                  | 0      | `read`              | 63      |

### Stdlib `SYS_*` enum (Cyrius `lib/syscalls.cyr`)

All x86_64. yukti references these via:

- `src/udev.cyr` — local `SYS_SOCKET=41, SYS_BIND=49, SYS_SETSOCKOPT=54,
  SYS_POLL=7, SYS_RECVFROM=45` plus stdlib `SYS_CLOSE`
- `src/optical.cyr` — stdlib `SYS_OPEN=2, SYS_CLOSE=3, SYS_IOCTL=16`
- `src/storage.cyr` — stdlib `SYS_OPEN=2, SYS_CLOSE=3, SYS_IOCTL=16`
- `src/partition.cyr` — stdlib `SYS_OPEN=2, SYS_CLOSE=3`
- `src/udev_rules.cyr` — stdlib `SYS_OPEN=2, SYS_CLOSE=3`

Every one of these must either come from an arch-aware stdlib or
be replaced with a yukti-owned per-arch constant block.

### Struct layouts that also differ

- `struct stat` — x86_64 and aarch64 both use the 64-bit layout
  from `asm-generic/stat.h`, but the `stat`/`lstat`/`fstat` entry
  points don't exist on aarch64, so the whole call pattern has to
  switch to `newfstatat(dirfd, path, &buf, flags)`. Offsets for
  `st_mode/uid/gid` in `device.cyr:157-159` need re-verifying
  under the `newfstatat` layout.
- `struct statfs` — same shape on x86_64 and aarch64 (generic),
  but `storage.cyr:126` calls syscall 137 directly — that's
  `statfs` on x86_64, `pkey_mprotect` on aarch64. aarch64 wants
  syscall 43 (`statfs`) with the identical argument layout.
- `struct sockaddr_in` / `sockaddr_nl` — arch-independent.
- `ioctl` request numbers (`CDROMEJECT`, `BLKGETSIZE64`, etc.) —
  defined in `include/uapi/linux/...`, same values across archs.
  The `syscall(SYS_IOCTL, ...)` sites are fine once `SYS_IOCTL`
  resolves to 29 on aarch64 instead of 16.

## Remediation options

### (A) Wait for stdlib arch dispatch

Cyrius stdlib already has `agnosys/1.0.0/lib/syscalls_agnos.cyr`
alongside `syscalls_linux.cyr`. Add a `syscalls_aarch64_linux.cyr`
peer and have `syscalls.cyr` select one at compile time based on
the `--aarch64` flag. Every consumer (yukti, sigil, bote, majra,
nous, agnosys) picks up the fix automatically.

- **Pro**: single point of fix, benefits every first-party
  project. Matches the "sovereign stack" posture — arch dispatch
  belongs in the stdlib, not scattered across consumers.
- **Con**: requires Cyrius toolchain work; yukti can't fix it
  unilaterally.

### (B) Yukti-local per-arch constants

Introduce `src/sys_linux_x86_64.cyr` and `src/sys_linux_aarch64.cyr`
in yukti with the full `SYS_*` plus yukti-specific extras
(`SYS_STAT`, `SYS_STATFS`, `SYS_MOUNT`, `SYS_UMOUNT2`,
`SYS_CLOCK_GETTIME`, etc.), pick one via the include chain in
`src/lib.cyr` based on a build flag, and rewrite every bare literal
in yukti to use the enum.

- **Pro**: unblocks yukti immediately without waiting on stdlib.
- **Con**: duplicates effort every downstream project will also
  have to do; two arch enums to keep in sync with the stdlib one
  if/when (A) lands.

### (C) Hybrid

Fix yukti locally with (B) for now; delete the local enum when
(A) lands. This is the pragmatic path if (A) is not imminent.

## When to re-run the retest

`scripts/retest-aarch64.sh pi` is the canonical reproducer. After
either remediation lands, every cross-built target should exit 0
on the Pi, including `yukti-test-aarch64`. The first run where
every line is "ok" is the signal that yukti can claim native
aarch64 support and the `held` status in CHANGELOG can flip to
`shipped`.

## Related / upstream

- See [2026-04-19-cc5-aarch64-repro.md](2026-04-19-cc5-aarch64-repro.md)
  for the historical SIGILL opcode bug — fixed in Cyrius 5.4.8.
- aarch64 generic syscall table: Linux kernel
  `include/uapi/asm-generic/unistd.h` (source of truth for the
  "aarch64" column in the table above).
- x86_64 syscall table: Linux kernel
  `arch/x86/entry/syscalls/syscall_64.tbl`.
