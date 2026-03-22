//! udev integration — device enumeration and hotplug monitoring.
//!
//! This module provides the interface for udev-based device discovery.
//! On Linux, it reads from `/sys/` and monitors udev events via netlink.
//! The actual netlink/libudev integration is behind trait abstractions
//! so it can be tested without root.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::device::{DeviceCapability, DeviceClass, DeviceId, DeviceInfo};
use crate::error::{Result, YantraError};

/// A raw udev device event from the kernel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UdevEvent {
    /// Action: "add", "remove", "change", "move", "bind", "unbind".
    pub action: String,
    /// Device path in sysfs.
    pub sys_path: PathBuf,
    /// Device node path (e.g. /dev/sdb1).
    pub dev_path: Option<PathBuf>,
    /// Subsystem (block, usb, scsi, etc).
    pub subsystem: String,
    /// Device type (disk, partition, etc).
    pub dev_type: Option<String>,
    /// All udev properties.
    pub properties: HashMap<String, String>,
}

impl UdevEvent {
    pub fn is_add(&self) -> bool {
        self.action == "add"
    }

    pub fn is_remove(&self) -> bool {
        self.action == "remove"
    }

    pub fn is_change(&self) -> bool {
        self.action == "change"
    }

    /// Get a property value.
    pub fn property(&self, key: &str) -> Option<&str> {
        self.properties.get(key).map(|s| s.as_str())
    }
}

/// Classify a device based on udev properties.
pub fn classify_device(event: &UdevEvent) -> DeviceClass {
    let bus = event.property("ID_BUS").unwrap_or("");
    let dev_type = event.dev_type.as_deref().unwrap_or("");
    let subsystem = event.subsystem.as_str();

    // USB storage
    if bus == "usb" && (dev_type == "disk" || dev_type == "partition") {
        return DeviceClass::UsbStorage;
    }

    // Optical drive
    if subsystem == "block" && event.property("ID_CDROM").is_some() {
        return DeviceClass::Optical;
    }

    // SD card
    if bus == "mmc" || event.property("ID_DRIVE_FLASH_SD").is_some() {
        return DeviceClass::SdCard;
    }

    // Device mapper
    if subsystem == "block" && event.sys_path.to_string_lossy().contains("/dm-") {
        return DeviceClass::DeviceMapper;
    }

    // Loop device
    if subsystem == "block" && event.sys_path.to_string_lossy().contains("/loop") {
        return DeviceClass::Loop;
    }

    // Internal block device (fallback for block subsystem)
    if subsystem == "block" && (dev_type == "disk" || dev_type == "partition") {
        return DeviceClass::BlockInternal;
    }

    DeviceClass::Unknown
}

/// Extract capabilities from udev properties.
pub fn extract_capabilities(event: &UdevEvent, class: DeviceClass) -> Vec<DeviceCapability> {
    let mut caps = vec![DeviceCapability::Read];

    // Writable unless read-only
    if event.property("ID_FS_READONLY").unwrap_or("0") != "1" {
        caps.push(DeviceCapability::Write);
    }

    // Removable
    let removable = event.property("ID_USB_DRIVER").is_some()
        || event.property("UDISKS_REMOVABLE").unwrap_or("0") == "1"
        || matches!(class, DeviceClass::UsbStorage | DeviceClass::Optical | DeviceClass::SdCard);
    if removable {
        caps.push(DeviceCapability::Removable);
        caps.push(DeviceCapability::Hotplug);
        caps.push(DeviceCapability::Eject);
    }

    // Optical-specific
    if class == DeviceClass::Optical {
        caps.push(DeviceCapability::MediaChange);
        caps.push(DeviceCapability::TrayControl);
        if event.property("ID_CDROM_CD_RW").is_some()
            || event.property("ID_CDROM_DVD_RW").is_some()
        {
            caps.push(DeviceCapability::Burn);
        }
    }

    // SSD TRIM
    if event.property("ID_ATA_FEATURE_SET_TRIM").is_some() {
        caps.push(DeviceCapability::Trim);
    }

    caps
}

/// Build a DeviceInfo from a udev event.
pub fn device_info_from_udev(event: &UdevEvent) -> Result<DeviceInfo> {
    let dev_path = event
        .dev_path
        .clone()
        .ok_or_else(|| YantraError::Udev("no dev_path in event".into()))?;

    let class = classify_device(event);
    let id = DeviceId::new(format!(
        "{}:{}",
        event.subsystem,
        dev_path.file_name().unwrap_or_default().to_string_lossy()
    ));

    let mut info = DeviceInfo::new(id, dev_path, class);
    info.sys_path = Some(event.sys_path.clone());
    info.vendor = event.property("ID_VENDOR").map(|s| s.to_string());
    info.model = event.property("ID_MODEL").map(|s| s.to_string());
    info.serial = event.property("ID_SERIAL_SHORT").map(|s| s.to_string());
    info.label = event
        .property("ID_FS_LABEL")
        .or_else(|| event.property("ID_FS_LABEL_ENC"))
        .map(|s| s.to_string());
    info.fs_type = event.property("ID_FS_TYPE").map(|s| s.to_string());

    if let Some(size_str) = event.property("ID_PART_ENTRY_SIZE") {
        if let Ok(sectors) = size_str.parse::<u64>() {
            info.size_bytes = sectors * 512;
        }
    }

    info.capabilities = extract_capabilities(event, class);
    info.properties = event.properties.clone();

    Ok(info)
}

/// Parse /sys/block/ to enumerate block devices.
///
/// In production, reads actual sysfs. This implementation provides the
/// enumeration logic given a sysfs root path.
pub fn enumerate_block_devices(sysfs_root: &Path) -> Vec<PathBuf> {
    let block_dir = sysfs_root.join("block");
    let mut devices = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&block_dir) {
        for entry in entries.flatten() {
            devices.push(entry.path());
        }
    }
    devices
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_usb_event() -> UdevEvent {
        let mut props = HashMap::new();
        props.insert("ID_BUS".into(), "usb".into());
        props.insert("ID_VENDOR".into(), "SanDisk".into());
        props.insert("ID_MODEL".into(), "Cruzer_Blade".into());
        props.insert("ID_SERIAL_SHORT".into(), "ABC123".into());
        props.insert("ID_FS_TYPE".into(), "vfat".into());
        props.insert("ID_FS_LABEL".into(), "MYUSB".into());
        props.insert("ID_USB_DRIVER".into(), "usb-storage".into());

        UdevEvent {
            action: "add".into(),
            sys_path: PathBuf::from("/sys/devices/pci0000:00/usb1/1-1/1-1:1.0/host0/target0:0:0/0:0:0:0/block/sdb/sdb1"),
            dev_path: Some(PathBuf::from("/dev/sdb1")),
            subsystem: "block".into(),
            dev_type: Some("partition".into()),
            properties: props,
        }
    }

    fn make_optical_event() -> UdevEvent {
        let mut props = HashMap::new();
        props.insert("ID_CDROM".into(), "1".into());
        props.insert("ID_CDROM_DVD".into(), "1".into());
        props.insert("ID_CDROM_DVD_RW".into(), "1".into());
        props.insert("ID_FS_TYPE".into(), "iso9660".into());
        props.insert("ID_FS_LABEL".into(), "MOVIE_DISC".into());

        UdevEvent {
            action: "change".into(),
            sys_path: PathBuf::from("/sys/devices/pci0000:00/ata1/host0/target0:0:0/0:0:0:0/block/sr0"),
            dev_path: Some(PathBuf::from("/dev/sr0")),
            subsystem: "block".into(),
            dev_type: Some("disk".into()),
            properties: props,
        }
    }

    #[test]
    fn test_classify_usb() {
        let event = make_usb_event();
        assert_eq!(classify_device(&event), DeviceClass::UsbStorage);
    }

    #[test]
    fn test_classify_optical() {
        let event = make_optical_event();
        assert_eq!(classify_device(&event), DeviceClass::Optical);
    }

    #[test]
    fn test_classify_loop() {
        let event = UdevEvent {
            action: "add".into(),
            sys_path: PathBuf::from("/sys/devices/virtual/block/loop0"),
            dev_path: Some(PathBuf::from("/dev/loop0")),
            subsystem: "block".into(),
            dev_type: Some("disk".into()),
            properties: HashMap::new(),
        };
        assert_eq!(classify_device(&event), DeviceClass::Loop);
    }

    #[test]
    fn test_usb_capabilities() {
        let event = make_usb_event();
        let caps = extract_capabilities(&event, DeviceClass::UsbStorage);
        assert!(caps.contains(&DeviceCapability::Read));
        assert!(caps.contains(&DeviceCapability::Removable));
        assert!(caps.contains(&DeviceCapability::Hotplug));
        assert!(caps.contains(&DeviceCapability::Eject));
    }

    #[test]
    fn test_optical_capabilities() {
        let event = make_optical_event();
        let caps = extract_capabilities(&event, DeviceClass::Optical);
        assert!(caps.contains(&DeviceCapability::TrayControl));
        assert!(caps.contains(&DeviceCapability::MediaChange));
        assert!(caps.contains(&DeviceCapability::Burn)); // DVD-RW
    }

    #[test]
    fn test_device_info_from_udev() {
        let event = make_usb_event();
        let info = device_info_from_udev(&event).unwrap();
        assert_eq!(info.class, DeviceClass::UsbStorage);
        assert_eq!(info.vendor.as_deref(), Some("SanDisk"));
        assert_eq!(info.model.as_deref(), Some("Cruzer_Blade"));
        assert_eq!(info.serial.as_deref(), Some("ABC123"));
        assert_eq!(info.label.as_deref(), Some("MYUSB"));
        assert_eq!(info.fs_type.as_deref(), Some("vfat"));
    }

    #[test]
    fn test_device_info_from_optical() {
        let event = make_optical_event();
        let info = device_info_from_udev(&event).unwrap();
        assert_eq!(info.class, DeviceClass::Optical);
        assert_eq!(info.label.as_deref(), Some("MOVIE_DISC"));
    }

    #[test]
    fn test_udev_event_actions() {
        let mut event = make_usb_event();
        assert!(event.is_add());
        assert!(!event.is_remove());

        event.action = "remove".into();
        assert!(event.is_remove());

        event.action = "change".into();
        assert!(event.is_change());
    }

    #[test]
    fn test_no_dev_path_error() {
        let event = UdevEvent {
            action: "add".into(),
            sys_path: PathBuf::from("/sys/test"),
            dev_path: None,
            subsystem: "block".into(),
            dev_type: None,
            properties: HashMap::new(),
        };
        assert!(device_info_from_udev(&event).is_err());
    }
}
