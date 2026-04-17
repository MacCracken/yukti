# Roadmap

## Completed (v1.0.0)

- [x] Full Cyrius port from Rust (April 2026)
- [x] 407 test assertions, 45 benchmarks, 2 fuzz targets
- [x] 152 KB static binary, zero dependencies
- [x] All 8 modules: error, device, event, storage, optical, udev, linux, udev_rules
- [x] Stdlib inclusion preparation (reviewed patra, sakshi)

## Completed (v1.1.0)

- [x] sakshi_full structured logging with spans (mount, eject, tray, TOC, enumerate, monitor)
- [x] patra device database — persist device history, mount counts, per-device preferences
- [x] Network filesystem mount helpers — SMB/CIFS and NFS/NFS4 with credentials, probing
- [x] Partition management — MBR and GPT table reading, EFI detection, boot flags
- [x] CI/release workflows ported to Cyrius (matching patra/sakshi pattern)
- [x] dist/yukti.cyr bundle for stdlib inclusion

## Completed (v1.1.0 → Cyrius 3.4.12)

- [x] Published yukti as Cyrius stdlib module (`lib/yukti.cyr`)
- [x] Extended `lib/str.cyr` with 12 new functions upstreamed to Cyrius stdlib
- [x] Added `[deps.yukti]` to cyrius manifest

## Completed (v1.2.0 → Cyrius 5.2.x modernization)

- [x] Migrated manifest `cyrius.toml` → `cyrius.cyml` with `${file:VERSION}`
- [x] Dropped `scripts/bundle.sh` — replaced by native `cyrius distlib`
- [x] Added `cyrius.lock` (SHA256 hashes) via `cyrius deps --lock`
- [x] CI + release workflows updated to cc5 and `cyrius deps --verify` gate
- [x] Switched `sakshi_full.cyr` → unified `sakshi.cyr` (sakshi 2.0.0)
- [x] Test suite grew from 407 → 485 assertions

## Next Release

### Dual-mode architecture (kernel + userland)
- [ ] Extract kernel-safe core module: device structs, PCI class/vendor enums, DeviceType constants — pure data, no syscalls
- [ ] Keep Linux sysfs/udev layer as optional module on top
- [ ] AGNOS kernel can consume the core module bare-metal for PCI device identification
- [ ] Userland (kybernet, argonaut) consumes full yukti with sysfs enumeration

## Medium Term

### Optical Enhancements
- [ ] Dual-layer/dual-sided disc type variants
- [ ] Audio CD ripping support (raw sector reads)

### Performance
- [ ] Eliminate bump allocator waste with targeted freelist for hot paths
- [ ] Pool DeviceInfo structs for enumeration (avoid per-device alloc)

## Long Term

### Ecosystem Integration
- [ ] jalwa — hotplug -> detect -> mount -> import pipeline
- [ ] argonaut — policy-driven automount on boot
- [ ] aethersafha — notifications for mount/unmount events

### Platform
- [ ] aarch64 support (cross-compile via Cyrius aarch64 backend)
- [ ] Container-aware enumeration (detect host vs container devices)
