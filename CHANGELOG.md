# Changelog

## [0.1.0] - 2026-03-22

### Added
- device: DeviceInfo, DeviceId, DeviceClass (8 types), DeviceCapability (10 types), DeviceState, Device trait
- event: DeviceEvent, DeviceEventKind, EventListener trait, EventCollector for testing
- storage: Filesystem enum (12 types), MountOptions, mount point validation, default mount point generation
- optical: DiscType (10 types), TrayState, DiscToc, TocEntry, disc type detection
- udev: UdevEvent parsing, device classification from udev properties, capability extraction, DeviceInfo construction from udev
- error: YantraError with 11 variants
- Feature gates: udev, storage, optical, ai
