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

## Completed (v1.3.0 → kernel-safe split + Cyrius 5.4.6)

- [x] Extract kernel-safe core module (`src/core.cyr`) — device structs, DeviceClass / DeviceState / DeviceCapabilities enums, pure accessors; no alloc, no syscalls
- [x] `src/pci.cyr` — PCI class/subclass/vendor/device tables with pure predicates (`pci_class_to_device_type`, `pci_is_storage`, `pci_is_gpu`, …)
- [x] `programs/core_smoke.cyr` invariant tripwire — catches accidental alloc/syscall additions to the kernel-safe subset
- [x] Multi-dist profiles via `[lib.core]` in `cyrius.cyml` — `cyrius distlib core` → `dist/yukti-core.cyr` (451 lines)
- [x] AGNOS kernel consumes `dist/yukti-core.cyr` bare-metal for PCI device identification
- [x] Userland (kybernet, argonaut) consumes the full `dist/yukti.cyr` bundle with sysfs enumeration
- [x] Toolchain pin bumped to Cyrius 5.4.6
- [x] `docs/development/cyrius-usage.md` as single source of truth for toolchain commands; `CLAUDE.md` restructured per agnosticos first-party template

## Next Release (v2.0.0 / v2.0.1)

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
