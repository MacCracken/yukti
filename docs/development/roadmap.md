# Roadmap

Forward-looking only. `CHANGELOG.md` is the authoritative record of
completed work — don't duplicate it here.

## Long Term

### Ecosystem Integration
- [ ] jalwa — hotplug → detect → mount → import pipeline
- [ ] argonaut — policy-driven automount on boot
- [ ] aethersafha — notifications for mount/unmount events

### Platform
- [ ] aarch64 native build (via Cyrius aarch64 backend)
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
