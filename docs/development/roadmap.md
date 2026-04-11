# Roadmap

## Completed (v1.0.0)

- [x] Full Cyrius port from Rust (April 2026)
- [x] 407 test assertions, 45 benchmarks, 2 fuzz targets
- [x] 152 KB static binary, zero dependencies
- [x] All 8 modules: error, device, event, storage, optical, udev, linux, udev_rules
- [x] Stdlib inclusion preparation (reviewed patra, sakshi)

## Next Release

### Cyrius Stdlib Integration
- [ ] Publish yukti as a Cyrius stdlib module (lib/yukti.cyr)
- [ ] Integration with sakshi_full structured logging
- [ ] Integration with patra for device database persistence

### Network Filesystem Mount Helpers
- [ ] SMB/NFS mount with credential support via `MountOptions`
- [ ] NAS autodiscovery (mDNS/SSDP)
- [ ] `DC_NETWORK` with actual SMB/NFS share detection

## Medium Term

### Partition Management
- [ ] Partition table reading module (MBR/GPT)
- [ ] EFI System Partition detection
- [ ] Boot flag and partition type queries

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
