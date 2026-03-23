# Roadmap

## Next Release

### Network Filesystem Mount Helpers
- [ ] SMB/NFS mount with credential support via `MountOptions`
- [ ] NAS autodiscovery (mDNS/SSDP/Avahi)
- [ ] `DeviceClass::Network` with actual SMB/NFS share detection

## Medium Term

### Partition Management
- [ ] Partition table reading module (MBR/GPT)
- [ ] EFI System Partition detection
- [ ] Boot flag and partition type queries

### Optical Enhancements
- [ ] Dual-layer/dual-sided disc type variants
- [ ] Audio CD ripping support (raw sector reads)

### AI Integration (`ai` feature)
- [ ] Device classification hints via LLM — identify unknown device types from udev properties
- [ ] Natural language device queries ("find all mounted USB drives larger than 8GB")
- [ ] Smart automount policies — learn user patterns for mount options
- [ ] Integration with daimon/hoosh agent runtime

## Long Term

### Ecosystem Integration
- [ ] jalwa — hotplug → detect → mount → import pipeline
- [ ] argonaut — policy-driven automount on boot
- [ ] aethersafha — D-Bus notifications for mount/unmount events

### Platform
- [ ] FreeBSD support (devd instead of udev, geom instead of sysfs)
- [ ] Container-aware enumeration (detect host vs container devices)
