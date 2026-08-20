# Roadmap

Forward-looking only. `CHANGELOG.md` is the authoritative record of
completed work — don't duplicate it here.

## Next patch — 2.3.9: audit follow-through

2.3.4 was the P(-1) audit / refactor / hardening / security sweep. The
findings it fixed are in `CHANGELOG.md`; the full write-up with severity,
file:line and refuted claims is
[`docs/audit/2026-08-19-audit.md`](../audit/2026-08-19-audit.md).

2.3.5 took the confidently-wrong-answer batch and the test-credibility
work; 2.3.6 audited every CI gate by deliberately breaking what each one
checks (5 of 16 were broken, and a 6th category had no gate at all);
2.3.7 closed the memory-retention theme; 2.3.8 took the
untrusted-input correctness batch. These are what remains. All were independently
re-verified; none is speculative. Everything here is a REPAIR: no item in
this list adds public surface, so it all belongs in the 2.3.x line.

### Memory retention — residue after 2.3.7

2.3.7 fixed the three findings under this heading (`DeviceInfo` recycle
list, `udev_monitor_run` per-event leak, `device_db_open` fd leak). Two
ownership questions it deliberately did NOT settle:

- [ ] **`DeviceInfo`'s `Str` fields are still bump-allocated and never
      reclaimed** — 88 bytes per device (a 24-byte device id plus 16 bytes
      per Str). `device_info_new` and the `di_set_*` setters store
      caller-allocated pointers **without copying**, so `device_info_free`
      cannot free them without double-freeing memory the caller may still
      hold. This is what `enumerate_devices_into(pool)` (2.5.0) is really
      for: it is filed there as an ergonomics idea and is actually the
      ownership model this needs. Retention went 272 -> 88 B/device in
      2.3.7; closing the last 88 needs that API.
- [ ] **The `DeviceEvent` dispatched to a listener is never freed.**
      `_udev_handle_event` frees it only on the filter-reject path.
      `fncall1(listener_fn, dev_event)` hands the pointer downstream and
      nothing frees it today, so a listener that RETAINS the event is
      currently correct — freeing after dispatch would be a use-after-free
      for every existing consumer. Settling this means documenting whether
      a listener may retain, which is a contract decision for the
      consumers (jalwa, argonaut, aethersafha, file manager), not a
      unilateral repair.
- [ ] **The `DeviceInfo` recycle list has no double-free detection.**
      A plain LIFO threaded through `DI_ID`; freeing the same block twice
      makes it cyclic. The stdlib freelist has poison mode, but routing
      `DeviceInfo` through `fl_alloc` was measured to cost 129 -> 243 ns on
      `device_info/create` because that path creates without freeing. If
      double-frees become a real risk, a cheap generation tag in an unused
      field would catch them without the allocator cost.

### Correctness — residue after 2.3.8

- [ ] **`filesystem_usage`'s statfs guards are not gated.** The real syscall
      will not return hostile values on demand, so the rejection paths
      (non-positive/implausible `f_bsize`, negative `f_blocks`, byte-product
      overflow) have no test that fails when they are removed. The test
      asserts a real mount reports sanely and checks the replacement scaling
      arithmetic directly, which is not the same thing. Gating this properly
      needs a seam — either a `_statfs_usage_from(buf)` split from the syscall
      (the shape used for `_udev_handle_event` and `_audio_parse_cards_text`),
      or a mock FUSE mount.
- [ ] **The GPT `entry_start_lba` guard is not gated.** Removing it still
      passes the fuzzer: a wild seek reads nothing, the loop breaks, and the
      observable outcome is identical to a rejected header. It is
      defense-in-depth against handing a wrapped negative offset to a seek.
      Gating it means observing the seek offset itself.
- [ ] **`str_from` does not copy, and yukti calls it on borrowed pointers
      elsewhere.** 2.3.8 fixed `device_db_get_preference`, where the borrowed
      pointer came from a patra result that was then freed. The same pattern —
      `str_from(<pointer into a buffer with a shorter lifetime>)` — deserves a
      sweep across `src/`: `_read_sysfs_attr` returns
      `str_trim(str_from_buf(&attr_buf, n))` over a STATIC buffer, and
      `str_from_buf` is `str_new_a`, which also does not copy. That has not
      been shown to be reachable as a bug, but it is the same shape and has
      not been checked.

### Hardening (defence-in-depth)### Hardening (defence-in-depth)

- [ ] **ANSI escape injection to the terminal.** `src/main.cyr:49` writes
      device-controlled sysfs/uevent strings verbatim.
- [ ] **`validate_mount_point` is a denylist, not an allowlist**
      (`src/storage.cyr:265`), and `/mnt-evil` passes a `/mnt` prefix test.
      `storage_unmount`'s `rmdir` gate (`:657`) likewise accepts `..` in
      the tail after a raw `/run/media/` prefix match.
- [ ] **SMB credentials are interpolated into the `mount(2)` data string**
      with no `,` / `=` rejection (`src/network.cyr:115`). Reachability was
      **not** traced to any consumer, so this is a library hardening gap,
      not a demonstrated exploit. Note the `credentials=` "arbitrary file
      read" variant is **refuted** — that is a mount.cifs userspace helper
      option and never reaches the syscall.
- [ ] **`network_mount` omits the mount-path TOCTOU lstat guard** that
      `storage_mount` has. `src/network.cyr:178`.
- [ ] **`lib/fs.cyr`'s `dir_list` / `is_dir` scratch is no longer
      reentrant.** 6.5.29 changed `var buf = alloc(4096)` to
      `var sbuf[4096]; var buf = &sbuf;` (`lib/fs.cyr:125, 178, 357, 378`).
      The stdlib comment justifies it as "per-call and therefore
      per-THREAD", but in Cyrius `var buf[N]` inside a function body is
      **static data** — `docs/development/cyrius-usage.md` says so
      explicitly. yukti's seven call sites are safe today (single-threaded,
      no nesting): `src/gpu.cyr:152`, `src/audio.cyr:428`,
      `src/udev.cyr:297, :393, :419`, `src/optical.cyr:603, :608`. The
      upside is a 4104 B/call bump-heap saving per enumeration, which
      matters for exactly the long-running consumers above. Recorded so the
      lost guarantee is a known constraint — verify before any nested or
      threaded enumeration lands.

### Structural / test quality

- [ ] **Adopt `defer { sakshi_span_exit(); }`.** Verified to run on every
      early-return path in cyrius 6.5.29, including from inside a loop, and
      recommended by sakshi's own header comment. Makes the 2.3.4 span-leak
      class structurally impossible instead of relying on 18 sites staying
      correct. Deferred because it also changes **which work falls inside
      the measured span** — `storage_unmount` and `network_mount` exit their
      span partway through and then do more work.
- [ ] **`core_smoke` asserts no struct-layout offset.**
      `programs/core_smoke.cyr:62`. It checks the exported constants but not
      the `DI_*` / `DH_*` offsets the AGNOS kernel actually depends on, so
      that ABI could shift without the tripwire firing.
- [ ] **The two `remove_rule` assertions added in 2.3.4 still pass either
      way** — with validation disabled `remove_rule` proceeds to `xunlink`
      a nonexistent path and still errors, so the assertion cannot tell
      "rejected by validation" from "unlink failed". The name gate itself
      is covered by the four `validate_rule` assertions. (The other
      cannot-fail assertions were replaced in 2.3.5.)
- [ ] **`pci_device_name` silently ignores `device_id`.** `src/pci.cyr`.
      The deferral is now cross-referenced and CI gates on that (2.3.6), but
      the API still takes an argument it discards — a caller cannot tell.
- [ ] **`storage_eject` duplicates `optical.cyr`'s eject path** with its own
      ioctl constant (`src/storage.cyr:695`); the nvme and mmcblk branches
      at `:708` are byte-identical.
- [ ] **`LDM_MONITOR_THREAD` is written once and never read.**
      `src/linux.cyr:22`.
- [ ] **4 raw `sys_open` sites with literal flags remain** (`audio.cyr` ×2,
      `storage.cyr`, `udev_rules.cyr`) — agnos's `sys_open` is
      `(name, namelen, flags)`. 2.3.4 cleared one via `read_procfs_text`.
- [ ] **`continue`-as-`break` sweep.** 2.3.5 measured `enumerate_devices`'
      partition loop running 1 of 5 iterations because `continue` aborted
      it, and fixed that loop by restructuring to nested `if`. **The
      trigger was never isolated** — six synthetic reproductions (var after
      the continue, nested loops, var loop bounds, allocation in the body,
      allocation in the guard, two guards) all behave correctly, so the
      minimal case is unknown. 27 other `continue` sites remain in `src/`;
      most have test or benchmark coverage demonstrating they iterate
      fully, and none was rewritten on a hypothesis. Verify each
      empirically — a counter in the real function, as 2.3.5 did — rather
      than by inspection. Worth an upstream cyrius report once a minimal
      repro exists.

### Upstream

- [ ] **`dist/yukti-core.deps` falsely lists `alloc`.** The kernel-safe
      bundle has zero call-shaped `alloc(`; cyrius's sidecar generator
      matches the word inside comments. **Confirmed by experiment** —
      rewording the two comments in `core.cyr` empties the sidecar. Not
      hand-correctable: `cyrius distlib` regenerates it and CI's dist-sync
      gate would fail. File upstream.

## Blocked on an upstream tag

- [ ] **Bump `[deps.sakshi]` 2.4.10 → 2.4.11.** The 2.3.4 span-leak work
      found a defect on sakshi's side too: `sakshi_span_enter` returned 0
      on overflow — the same value it returns on success — and emitted
      nothing, so a saturated span stack stopped ALL tracing with no
      diagnostic anywhere. Fixed in sakshi 2.4.11: the enter now returns a
      non-zero cumulative drop count when refused, and the first drop emits
      a one-shot `sakshi_warn`. Pairing semantics are deliberately
      unchanged (that is a 2.5.0 minor — it alters observable behaviour for
      unbalanced callers, and sakshi is included by every AGNOS Cyrius
      project). sakshi's own pin also moved 6.5.15 → 6.5.29.

      **The change is committed locally but not tagged.** Bump the tag here
      once 2.4.11 is pushed, then regenerate `cyrius.lock`. Keep it in
      lockstep with whatever cyrius bundles, or `cyrius build` warns that
      `./lib/` shadows the version-pinned lib.

## Resolved in 2.3.4

- [x] **`st_mode` read at the x86_64 offset on every architecture.** The
      2.3.3 filing named only `src/storage.cyr:575` (the TOCTOU symlink
      guard, which on aarch64 compared a **uid** against `S_IFLNK` and so
      could never fire). The 2.3.4 sweep found a **second instance** —
      `src/device.cyr:157` `query_permissions`, flagged independently by
      three dimensions, reading st_uid as the mode and st_gid as the uid on
      aarch64 — and that a *third* fact made it a pattern rather than a
      slip: `src/` contained **zero** named `STAT_*` constants. Every stat
      offset in the tree is now the stdlib's per-target constant. The agnos
      build then caught what tests could not: agnos's `struct stat` has no
      uid/gid field at all, so `query_permissions` fails closed there
      rather than fabricating 0/0.

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
