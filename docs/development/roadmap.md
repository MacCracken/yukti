# Roadmap

Forward-looking only. `CHANGELOG.md` is the authoritative record of
completed work — don't duplicate it here.

## Next patch — 2.3.2: raw syscall review and cleanup

Finishes the job 2.1.4 started. That release migrated all 33
arch-divergent raw `syscall(N, …)` sites in `src/` to stdlib
wrappers and `SYS_*` constants — and stopped at the `src/`
boundary. Everything else still hardcodes x86_64 Linux numbers.

Current inventory (40 live sites, comments excluded; `src/` is
clean and must stay that way):

| Location                         | Sites |
|----------------------------------|-------|
| `tests/tcyr/yukti.tcyr`          | 21    |
| `fuzz/fuzz_partition_table.fcyr` | 8     |
| `tests/bcyr/yukti.bcyr`          | 4     |
| `programs/core_smoke.cyr`        | 3     |
| `fuzz/fuzz_parse_uevent.fcyr`    | 2     |
| `fuzz/fuzz_mount_table.fcyr`     | 2     |

By number: `syscall(87)` unlink ×21, `syscall(1)` write ×9,
`syscall(60)` exit ×8, `syscall(2)` open ×1, `syscall(3)` close ×1.

None of those numbers mean what the code intends off x86_64.
On aarch64 the correct values are write 64, exit 93, unlinkat 35,
openat 56, close 57 — so every one of the 40 currently calls
something unrelated. On agnos, `87` is `SYS_GPU_BLIT_SHM` (a GPU
DMA op) and `60` is `SYS_WINSIZE`; only `syscall(1)` happens to
coincide with `SYS_WRITE`.

Two of these actively falsify a gate, which is what makes this a
patch and not a cleanup nicety:

- [ ] **`programs/core_smoke.cyr` cannot fail on aarch64** — the
      kernel-safe tripwire's abort path is `syscall(60, 1)` and
      its output path is `syscall(1, …)`, both x86-only. On
      aarch64 it neither prints nor exits, falls through every
      remaining assertion, and terminates 0 regardless of whether
      the invariant holds. `scripts/retest-aarch64.sh` gates
      purely on that exit code. CI is unaffected — it builds the
      aarch64 binary but only *runs* the x86_64 one.
- [ ] **`fuzz/fuzz_partition_table.fcyr` fuzzes nothing on
      aarch64** — it writes its fixture with raw open/write/close,
      so on aarch64 the fixture is never created and the harness
      still exits 0.
- [ ] **Migrate the remaining 40 sites** to `sys_write` /
      `sys_exit` / `xunlink` / `xopen` / `sys_close`, which
      arch-dispatch correctly. Prefer the `x*` portable wrappers
      where one exists — they also handle the agnos
      length-carrying ABI, which is what `_yk_mkdir` and `xrmdir`
      exist for on the `src/` side.
- [ ] **Add a CI grep gate** rejecting new numeric-literal
      `syscall(` outside comments, so `src/` cannot regress and
      the rest cannot drift back. The existing security-scan job
      is the natural home.
- [ ] **Check the three discarded write returns** —
      `udev_rules.cyr:147-148` (rule file) and `storage.cyr:773`
      (sysfs eject/delete) drop `sys_write`'s return, so a short
      or failed write is silent. The other ~38 discarded returns
      in `src/` are deliberate (`sys_close`, stdout writes,
      `_yk_mkdir` where `EEXIST` is expected) and should stay.
      Noted in `docs/development/threat-model.md`.

Not in scope: the 9001+ agnos stub band in `src/syscalls.cyr` is
deliberately numeric and deliberately invalid — see the comment
there before touching it.

## Next minor — 2.4.0: security hardening + prepared statements

Second pass on the 2026-04-19 P(-1) audit. Pulls in cyrius
`lib/security.cyr` Landlock + `lib/random.cyr` getrandom
surfaces (flagged at 2.1.4 ship time as the next defense-in-depth
opportunity).

- [ ] **`open_tree(2)` + `move_mount(2)` atomic mount path** —
      closes the narrow TOCTOU window that `newfstatat` still
      leaves open (audit MED-2). Replaces the current
      open-then-mount sequence in `src/storage.cyr`.
- [ ] **Landlock filesystem-access rules on mount points** —
      restricts the mount syscall's filesystem write surface
      using `lib/security.cyr` (Cyrius 5.7.35). Layered on top
      of the existing path validation.
- [ ] **`device_db` SQL layer → patra prepared statements** —
      replaces the string-escape defense from 2.0.0 audit
      HIGH-1. Depends on patra growing a `patra_bind_*` API;
      track upstream.

## Future minor — 2.5.0: device-shape extensions

- [ ] Container-aware enumeration (host vs container devices)
- [ ] M.2 / NVMe namespace reporting beyond the single-namespace
      default
- [ ] Bulk DeviceInfo pool with `enumerate_devices_into(pool)` —
      needs caller lifecycle cooperation; investigate alongside
      jalwa / argonaut integration to understand realistic
      consumer patterns.

## Held — hardware-bound

- [ ] **aarch64 native build — runtime SIGILL retest on
      Cortex-A72** against the current **6.5.3** toolchain.
      `src/` is cross-build-clean and runtime-correct as of 2.1.4
      (33 raw-number arch-divergent syscalls migrated to wrappers
      / `SYS_*` constants; `src/syscalls.cyr` arch-conditional
      layer + ppoll-uniform poll path; udev local-enum
      shadowing dropped). The Cortex-A72 Linux SIGILL repro from
      Cyrius 5.4.6 has still not been re-run — it has now gone
      un-retested across the whole 5.5.x → 6.5.x arc.
      Hardware-bound, not a code change.

      Two prerequisites moved since this was written:
      the backend binary was renamed `cc5_aarch64` →
      `cycc_aarch64` in Cyrius 6.0, and `scripts/retest-aarch64.sh`
      probed only the old name — so it exited 2 before doing any
      work on every 6.x toolchain. Fixed in 2.3.1; the script now
      accepts either name.

      **Sequence 2.3.2 first.** The retest gates purely on remote
      exit code, and `core_smoke` plus `fuzz_partition_table`
      cannot report failure on aarch64 until their raw x86-only
      exit/write paths are migrated — so a retest run today would
      report success no matter what the hardware did.
      See `docs/development/issues/2026-04-19-cc5-aarch64-repro.md`
      and `scripts/retest-aarch64.sh`.

## Toolchain integration opportunities

Small infrastructure wins. All three were re-verified present
under the current 6.5.3 pin (2026-07-30) — they are available
now, not pending an upgrade. Could land in any patch slot:

- [ ] **`cyrius smoke`** — replaces the manual build-then-run
      dance for `programs/core_smoke.cyr` with the upstream test
      harness. Pairs naturally with 2.3.2, which has to touch
      `core_smoke`'s exit path anyway.
- [ ] **`cyrius api-surface`** — public-API diff gate. Formalises
      yukti's stable surface for AGNOS kernel / jalwa / argonaut /
      aethersafha / vani consumers and catches accidental breaking
      changes at PR time. Present but undocumented in `--help`;
      needs a `docs/api-surface.snapshot` seeded via
      `cyrius api-surface --update`.
- [ ] **`lib/test.cyr` `test_each`** — table-driven dispatch could
      compress homogeneous blocks of `tests/tcyr/yukti.tcyr`
      (e.g. `test_audio_parse_pcm_rejects_invalid`'s 12-case
      sweep, the disc-type predicates, the filesystem
      string-roundtrip tests).

## Ecosystem integration tracking

Downstream consumer status — yukti-side API is in place; these
track the consumer-side work and which yukti release unblocked
each:

| Consumer    | Integration                                        | Status                                |
|-------------|----------------------------------------------------|---------------------------------------|
| jalwa       | Hotplug → detect → mount → import pipeline         | yukti API ready (block/optical)       |
| argonaut    | Policy-driven automount on boot                    | yukti API ready                       |
| aethersafha | Notifications for mount/unmount events             | yukti API ready                       |
| vani        | Audio device discovery → descriptor → open         | unblocked by 2.2.0 — vani 0.3.x ready |
| AGNOS kernel| `dist/yukti-core.cyr` PCI tables + DeviceClass     | shipping since 2.0.0                  |

## Future / unscheduled

Ideas we're not committing to yet — park here if interesting
but not scheduled.

- [ ] Optional compression of mount history records in patra
