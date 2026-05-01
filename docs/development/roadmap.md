# Roadmap

Forward-looking only. `CHANGELOG.md` is the authoritative record of
completed work — don't duplicate it here.

## Next minor — 2.3.0: security hardening + prepared statements

Second pass on the 2026-04-19 P(-1) audit. Pulls in cyrius
v5.7.35's `lib/security.cyr` Landlock + `lib/random.cyr` getrandom
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

## Future minor — 2.4.0: device-shape extensions

- [ ] Container-aware enumeration (host vs container devices)
- [ ] M.2 / NVMe namespace reporting beyond the single-namespace
      default
- [ ] Bulk DeviceInfo pool with `enumerate_devices_into(pool)` —
      needs caller lifecycle cooperation; investigate alongside
      jalwa / argonaut integration to understand realistic
      consumer patterns.

## Held — hardware-bound

- [ ] **aarch64 native build — runtime SIGILL retest on
      Cortex-A72** with the 5.7.48 toolchain. Yukti is
      cross-build-clean and runtime-correct as of 2.1.4 (33
      raw-number arch-divergent syscalls migrated to wrappers
      / `SYS_*` constants; `src/syscalls.cyr` arch-conditional
      layer + ppoll-uniform poll path; udev local-enum
      shadowing dropped). The Cortex-A72 Linux SIGILL repro
      from Cyrius 5.4.6 has not yet been re-run against the
      meaningful aarch64 fixes the 5.5.x → 5.7.x arc landed
      (EW alignment assert v5.4.19, Apple Silicon Mach-O
      probe v5.5.11, f64 basic ops v5.7.30, EB() codebuf cap
      raised v5.7.34). Hardware-bound, not a code change.
      See `docs/development/issues/2026-04-19-cc5-aarch64-repro.md`
      and `scripts/retest-aarch64.sh`.

## Toolchain integration opportunities

Small infrastructure wins flagged during the 5.7.x toolchain pin
arc. Could land in any patch slot:

- [ ] **`cyrius smoke`** (v5.7.38) — replaces the manual
      build-then-run dance for `programs/core_smoke.cyr` with
      the upstream test harness.
- [ ] **`cyrius api-surface`** (v5.7.33) — public-API diff gate.
      Formalises yukti's stable surface for AGNOS kernel /
      jalwa / argonaut / aethersafha / vani consumers and
      catches accidental breaking changes at PR time.
- [ ] **`lib/test.cyr` `test_each`** (v5.7.43) — table-driven
      dispatch could compress homogeneous blocks of
      `tests/tcyr/yukti.tcyr` (e.g. `test_audio_parse_pcm_rejects_invalid`'s
      12-case sweep, the disc-type predicates, the filesystem
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
