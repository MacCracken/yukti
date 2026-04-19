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

## Completed (v2.0.0 → P(-1) security audit)

- [x] Full P(-1) security audit with CVE sweep (2024–2026 adjacent surfaces) — report at `docs/audit/2026-04-19-audit.md`
- [x] HIGH-1: SQL injection fix — `_sql_escape_str` helper applied to every `patra_exec`/`patra_query` call in `device_db.cyr`
- [x] HIGH-2: GPT stack buffer overflow fix — reject `entry_size != 128` in `read_partition_table`
- [x] MED-1: extended `_is_forbidden_mount` blacklist with prefix matching + new protected roots + `..`/`//` traversal rejection
- [x] MED-2: TOCTOU guard via `newfstatat(AT_SYMLINK_NOFOLLOW)` before `mount(2)`
- [x] MED-3: chunked reader for `/proc/mounts` (1 MB cap) — no more silent truncation on container/btrfs hosts
- [x] MED-4: netlink `recvfrom` with `sockaddr_nl` + `nl_pid == 0` sender check
- [x] MED-5: udevadm wrappers switched to absolute `/usr/bin/udevadm` + `exec_vec` argv lists; sysfs path gate on `trigger_device`/`query_device`
- [x] `fuzz/fuzz_partition_table.fcyr` — closes audit-flagged GPT coverage gap; explicit HIGH-2 regression check
- [x] 28 new security regression assertions (tcyr: 531 → 559)

## Next Release (v2.1.0 — remaining audit cleanup)

Low-severity findings from the 2026-04-19 audit, scheduled for 2.1.0:

- [ ] LOW-1: `/sys/block/<base>/device/delete` eject write — use `openat(dirfd, ..., O_NOFOLLOW)` under `/sys/block/` instead of full-path `SYS_OPEN`
- [ ] LOW-2: TOC integer overflow — cast to i64 before `length * 100` and `leadout_lba * 2048` in `read_toc`
- [ ] LOW-3: trailing-slash mount blacklist — `_is_forbidden_mount("/etc/")` should match (superseded by MED-1 prefix match but adding canonical-form guard for belt-and-braces)
- [ ] LOW-4: cap sanitized label length at 64 chars in `default_mount_point`
- [ ] LOW-5: `sakshi_warn` on empty `ACTION`/`DEVPATH` in `parse_uevent` for incident-response observability
- [ ] LOW-6: rewrite `docs/development/threat-model.md` for Cyrius era (current version still references `unsafe`, `cargo-deny`, `Option<String>`, `bitflags` — Rust-era leftovers)

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
