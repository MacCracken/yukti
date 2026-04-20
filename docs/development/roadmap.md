# Roadmap

Forward-looking only. `CHANGELOG.md` is the authoritative record of
completed work — don't duplicate it here.

## Long Term

### Ecosystem Integration
- [ ] jalwa — hotplug → detect → mount → import pipeline
- [ ] argonaut — policy-driven automount on boot
- [ ] aethersafha — notifications for mount/unmount events

### Platform
- [ ] aarch64 native build — cross-compile path is wired, but
      Cyrius 5.4.6's `cc5_aarch64` emitted an unallocated ARMv8-A
      opcode (`0x800000d6`) that `SIGILL`ed on real hardware.
      Needs retest on the 5.5.11 toolchain (Cyrius 5.4.19 added
      an `EW` alignment assert + v5.5.11 shipped an Apple Silicon
      Mach-O probe — both touch aarch64 codegen; Cortex-A72 Linux
      repro has not yet been re-run). See
      `docs/development/issues/2026-04-19-cc5-aarch64-repro.md` and
      `scripts/retest-aarch64.sh`.
- [ ] Container-aware enumeration (detect host vs container devices)

## Future

Ideas we're not committing to yet — park here if they're
interesting but not scheduled.

- [ ] Refactor `device_db` SQL layer to patra prepared statements
      once patra grows a `patra_bind_*` API (replaces the string-
      escape defense from 2.0.0 HIGH-1)
- [ ] `open_tree(2)` + `move_mount(2)` path for atomic mount
      (closes the narrow TOCTOU window that `newfstatat` still
      leaves open in 2.0.0 MED-2)
- [ ] Bulk DeviceInfo pool with `enumerate_devices_into(pool)` —
      needs caller lifecycle cooperation, investigate with jalwa
      / argonaut integration
- [ ] Optional compression of mount history records in patra
- [ ] M.2 / NVMe namespace reporting beyond the single-namespace
      default
