# Roadmap

Forward-looking only. `CHANGELOG.md` is the authoritative record of
completed work — don't duplicate it here.

## Next patch — 2.3.4: audit sweep and review

Findings from the 2.3.3 dependency-upgrade audit that were deliberately
**not** fixed in 2.3.3, to keep that release scoped to the bump and the
one regression the bump introduced. None is caused by the upgrade;
all four predate it.

- [ ] **`mount_count` never increments, and it takes `last_seen` down
      with it.** `src/device_db.cyr:321` emits
      `UPDATE devices SET mount_count = mount_count + 1, last_seen = <now>`.
      patra's `_parse_update` (`lib/patra.cyr:2237`) accepts only
      `SQLT_INT_LIT`, `SQLT_PARAM` or `SQLT_STR_LIT` after `=`; a column
      reference falls through to the `else` and returns
      `PATRA_ERR_SYNTAX`, so the **whole statement** fails and `last_seen`
      is collateral damage.

      2.3.3's `_db_exec` made this audible — the suite now prints
      `[WARN] device_db: devices mount_count update failed` exactly once,
      from `test_device_db_mount_count`. Before that it was silent.

      The test at `tests/tcyr/yukti.tcyr:1768` documents the bug instead
      of catching it: *"mount_count UPDATE may not work … just verify the
      function doesn't crash"* followed by `assert(mc >= 0)`, which holds
      for every possible value. Fix: read the count, then write a literal
      (or use `patra_bind_int`, see 2.4.0 — the API is available now).
      Replace the placeholder assertion with a real one.
- [ ] **`st_mode` is read at the x86_64 offset on every architecture.**
      `src/storage.cyr:575` does `load32(&stbuf + 24)`. That is
      `STAT_MODE = 24` on x86_64 (`lib/syscalls_x86_64_linux.cyr:301`) but
      aarch64-Linux puts `st_mode` at **16**
      (`lib/syscalls_aarch64_linux.cyr:533`) — 24 is `st_uid` there. So
      the TOCTOU symlink guard added for audit MED-2 cannot fire on
      aarch64: it tests a uid against `S_IFLNK`. Security-relevant, and it
      should use the named `STAT_MODE` constant rather than a literal.
      Pairs with the held aarch64 retest.
- [ ] **`lib/fs.cyr`'s `dir_list` / `is_dir` scratch is no longer
      reentrant.** 6.5.29 changed `var buf = alloc(4096)` to
      `var sbuf[4096]; var buf = &sbuf;` (`lib/fs.cyr:125, 178, 357, 378`).
      The stdlib comment justifies it as "per-call and therefore
      per-THREAD" — but in Cyrius `var buf[N]` inside a function body is
      **static data**, which `docs/development/cyrius-usage.md` states
      explicitly. yukti's seven call sites are safe today (single-threaded,
      no nesting): `src/gpu.cyr:152`, `src/audio.cyr:428`,
      `src/udev.cyr:297, :393, :419`, `src/optical.cyr:603, :608`. The
      upside is a 4104 B/call bump-heap saving on every enumeration, which
      matters for the long-running consumers. Recorded so the lost
      guarantee is a known constraint rather than a surprise — verify
      before any nested or threaded enumeration lands.
- [ ] **`dist/yukti-core.deps` claims the kernel-safe bundle needs
      `alloc`.** It does not — `dist/yukti-core.cyr` has zero call-shaped
      `alloc(`. cyrius's sidecar generator matches the word inside
      comments (lines 17 and 100 of the bundle). Confirmed by experiment:
      rewording those two comments empties the sidecar. Misleading for the
      AGNOS kernel consumer, which reads that file to know what to bring
      into scope. Cannot be hand-corrected — `cyrius distlib` regenerates
      it and CI's dist-sync gate would fail. **Upstream cyrius issue**;
      file it. `programs/core_smoke.cyr`, the real tripwire, passes.

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
      HIGH-1. **Unblocked** — this said "depends on patra growing
      a `patra_bind_*` API; track upstream", and tracking upstream
      during the 2.3.3 dep bump found the API had already shipped.
      `patra_prepare` / `patra_bind_int` / `patra_bind_text` /
      `patra_finalize` are present in 1.13.8 at `lib/patra.cyr:6244,
      6467, 6482, 6277` — and were equally present in the *previously
      pinned* 1.12.12 (`5549, 5772, 5787`), so the item was never
      actually blocked at the pin it was written against.
      `patra_exec_prepared` / `patra_query_prepared` round out the
      surface. No upstream dependency remains; this is schedulable
      work whenever 2.4.0 opens.

## Future minor — 2.5.0: device-shape extensions

- [ ] Container-aware enumeration (host vs container devices)
- [ ] M.2 / NVMe namespace reporting beyond the single-namespace
      default
- [ ] Bulk DeviceInfo pool with `enumerate_devices_into(pool)` —
      needs caller lifecycle cooperation; investigate alongside
      jalwa / argonaut integration to understand realistic
      consumer patterns.

## Held — hardware-bound

- [ ] **`filesystem_usage()` returns EFAULT on aarch64.** Opened by
      2.3.2, which made the aarch64 suite report honestly for the
      first time. `statfs` fails with `-14` where `newfstatat` on
      the *same* static buffer returns 0 — so the buffer is valid
      and the fault is specific to `SYS_STATFS` (43, the correct
      aarch64 number). Two assertions fail: `fs usage root ok` and
      `avail > 0`; the other 656 pass.

      Observed under `qemu-aarch64` only and **not confirmed on real
      hardware** — qemu-user's statfs emulation is a plausible
      culprit, so reproduce on a Cortex-A72 before treating it as a
      yukti bug. Fold into the retest below.
- [ ] **aarch64 native build — runtime SIGILL retest on
      Cortex-A72** against the current **6.5.29** toolchain.
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

      2.3.2 cleared the prerequisite: the aarch64 suite now reports
      all 658 assertions instead of 182, so a retest can be believed.
      See `docs/development/issues/2026-04-19-cc5-aarch64-repro.md`
      and `scripts/retest-aarch64.sh`.

## Toolchain integration opportunities

Small infrastructure wins. All three were re-verified present
under the current 6.5.29 pin (2026-08-19, during the 2.3.3 bump)
— they are available now, not pending an upgrade. Could land in
any patch slot:

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
