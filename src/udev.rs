//! udev integration — device enumeration and hotplug monitoring.
//!
//! This module provides the interface for udev-based device discovery.
//! On Linux, it reads from `/sys/` and monitors udev events via netlink.
//! The actual netlink/libudev integration is behind trait abstractions
//! so it can be tested without root.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::device::{DeviceCapabilities, DeviceClass, DeviceId, DeviceInfo};
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
    #[inline]
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

    // Device mapper — use byte-level contains to avoid to_string_lossy allocation
    if subsystem == "block" {
        let path_bytes = event.sys_path.as_os_str().as_encoded_bytes();
        if contains_bytes(path_bytes, b"/dm-") {
            return DeviceClass::DeviceMapper;
        }
        if contains_bytes(path_bytes, b"/loop") {
            return DeviceClass::Loop;
        }
        if dev_type == "disk" || dev_type == "partition" {
            return DeviceClass::BlockInternal;
        }
    }

    DeviceClass::Unknown
}

/// Byte-level substring search (avoids string allocation from OsStr).
#[inline]
fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

/// Classify device and extract capabilities in a single pass,
/// avoiding redundant HashMap lookups.
pub fn classify_and_extract(event: &UdevEvent) -> (DeviceClass, DeviceCapabilities) {
    let class = classify_device(event);
    let caps = extract_capabilities(event, class);
    (class, caps)
}

/// Extract capabilities from udev properties.
pub fn extract_capabilities(event: &UdevEvent, class: DeviceClass) -> DeviceCapabilities {
    let mut caps = DeviceCapabilities::READ;

    // Writable unless read-only
    if event.property("ID_FS_READONLY").unwrap_or("0") != "1" {
        caps |= DeviceCapabilities::WRITE;
    }

    // Removable
    let removable = event.property("ID_USB_DRIVER").is_some()
        || event.property("UDISKS_REMOVABLE").unwrap_or("0") == "1"
        || matches!(
            class,
            DeviceClass::UsbStorage | DeviceClass::Optical | DeviceClass::SdCard
        );
    if removable {
        caps |= DeviceCapabilities::REMOVABLE
            | DeviceCapabilities::HOTPLUG
            | DeviceCapabilities::EJECT;
    }

    // Optical-specific
    if class == DeviceClass::Optical {
        caps |= DeviceCapabilities::MEDIA_CHANGE | DeviceCapabilities::TRAY_CONTROL;
        if event.property("ID_CDROM_CD_RW").is_some()
            || event.property("ID_CDROM_DVD_RW").is_some()
        {
            caps |= DeviceCapabilities::BURN;
        }
    }

    // SSD TRIM
    if event.property("ID_ATA_FEATURE_SET_TRIM").is_some() {
        caps |= DeviceCapabilities::TRIM;
    }

    caps
}

/// Build a DeviceInfo from a udev event.
pub fn device_info_from_udev(event: &UdevEvent) -> Result<DeviceInfo> {
    let dev_path = event
        .dev_path
        .clone()
        .ok_or_else(|| YantraError::Udev("no dev_path in event".into()))?;

    let (class, capabilities) = classify_and_extract(event);
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

    info.capabilities = capabilities;
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
    use crate::device::DeviceCapability;

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
    fn test_classify_dm() {
        let event = UdevEvent {
            action: "add".into(),
            sys_path: PathBuf::from("/sys/devices/virtual/block/dm-0"),
            dev_path: Some(PathBuf::from("/dev/dm-0")),
            subsystem: "block".into(),
            dev_type: Some("disk".into()),
            properties: HashMap::new(),
        };
        assert_eq!(classify_device(&event), DeviceClass::DeviceMapper);
    }

    #[test]
    fn test_classify_sd_card() {
        let mut props = HashMap::new();
        props.insert("ID_BUS".into(), "mmc".into());
        let event = UdevEvent {
            action: "add".into(),
            sys_path: PathBuf::from("/sys/devices/mmc0/mmc0:0001/block/mmcblk0"),
            dev_path: Some(PathBuf::from("/dev/mmcblk0")),
            subsystem: "block".into(),
            dev_type: Some("disk".into()),
            properties: props,
        };
        assert_eq!(classify_device(&event), DeviceClass::SdCard);
    }

    #[test]
    fn test_classify_block_internal() {
        let event = UdevEvent {
            action: "add".into(),
            sys_path: PathBuf::from("/sys/devices/pci0000:00/0000:00:17.0/ata1/host0/target0:0:0/0:0:0:0/block/sda"),
            dev_path: Some(PathBuf::from("/dev/sda")),
            subsystem: "block".into(),
            dev_type: Some("disk".into()),
            properties: HashMap::new(),
        };
        assert_eq!(classify_device(&event), DeviceClass::BlockInternal);
    }

    #[test]
    fn test_classify_unknown() {
        let event = UdevEvent {
            action: "add".into(),
            sys_path: PathBuf::from("/sys/devices/usb/input0"),
            dev_path: None,
            subsystem: "input".into(),
            dev_type: None,
            properties: HashMap::new(),
        };
        assert_eq!(classify_device(&event), DeviceClass::Unknown);
    }

    #[test]
    fn test_usb_capabilities() {
        let event = make_usb_event();
        let caps = extract_capabilities(&event, DeviceClass::UsbStorage);
        assert!(caps.contains(DeviceCapabilities::READ));
        assert!(caps.contains(DeviceCapabilities::REMOVABLE));
        assert!(caps.contains(DeviceCapabilities::HOTPLUG));
        assert!(caps.contains(DeviceCapabilities::EJECT));
        assert!(!caps.contains(DeviceCapabilities::TRAY_CONTROL));
    }

    #[test]
    fn test_optical_capabilities() {
        let event = make_optical_event();
        let caps = extract_capabilities(&event, DeviceClass::Optical);
        assert!(caps.contains(DeviceCapabilities::TRAY_CONTROL));
        assert!(caps.contains(DeviceCapabilities::MEDIA_CHANGE));
        assert!(caps.contains(DeviceCapabilities::BURN)); // DVD-RW
    }

    #[test]
    fn test_readonly_device() {
        let mut props = HashMap::new();
        props.insert("ID_FS_READONLY".into(), "1".into());
        let event = UdevEvent {
            action: "add".into(),
            sys_path: PathBuf::from("/sys/devices/block/sda"),
            dev_path: Some(PathBuf::from("/dev/sda")),
            subsystem: "block".into(),
            dev_type: Some("disk".into()),
            properties: props,
        };
        let caps = extract_capabilities(&event, DeviceClass::BlockInternal);
        assert!(caps.contains(DeviceCapabilities::READ));
        assert!(!caps.contains(DeviceCapabilities::WRITE));
    }

    #[test]
    fn test_trim_capability() {
        let mut props = HashMap::new();
        props.insert("ID_ATA_FEATURE_SET_TRIM".into(), "1".into());
        let event = UdevEvent {
            action: "add".into(),
            sys_path: PathBuf::from("/sys/devices/block/sda"),
            dev_path: Some(PathBuf::from("/dev/sda")),
            subsystem: "block".into(),
            dev_type: Some("disk".into()),
            properties: props,
        };
        let caps = extract_capabilities(&event, DeviceClass::BlockInternal);
        assert!(caps.contains(DeviceCapabilities::TRIM));
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
        assert!(info.has_capability(DeviceCapability::Removable));
    }

    #[test]
    fn test_device_info_from_optical() {
        let event = make_optical_event();
        let info = device_info_from_udev(&event).unwrap();
        assert_eq!(info.class, DeviceClass::Optical);
        assert_eq!(info.label.as_deref(), Some("MOVIE_DISC"));
        assert!(info.has_capability(DeviceCapability::TrayControl));
    }

    #[test]
    fn test_device_info_with_size() {
        let mut event = make_usb_event();
        event.properties.insert("ID_PART_ENTRY_SIZE".into(), "31457280".into());
        let info = device_info_from_udev(&event).unwrap();
        assert_eq!(info.size_bytes, 31457280 * 512);
    }

    #[test]
    fn test_device_info_label_enc_fallback() {
        let mut props = HashMap::new();
        props.insert("ID_BUS".into(), "usb".into());
        props.insert("ID_USB_DRIVER".into(), "usb-storage".into());
        props.insert("ID_FS_LABEL_ENC".into(), "ENCODED_LABEL".into());

        let event = UdevEvent {
            action: "add".into(),
            sys_path: PathBuf::from("/sys/devices/usb/block/sdb/sdb1"),
            dev_path: Some(PathBuf::from("/dev/sdb1")),
            subsystem: "block".into(),
            dev_type: Some("partition".into()),
            properties: props,
        };
        let info = device_info_from_udev(&event).unwrap();
        assert_eq!(info.label.as_deref(), Some("ENCODED_LABEL"));
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
    fn test_udev_event_property_missing() {
        let event = make_usb_event();
        assert!(event.property("NONEXISTENT_KEY").is_none());
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

    #[test]
    fn test_classify_and_extract_combined() {
        let event = make_usb_event();
        let (class, caps) = classify_and_extract(&event);
        assert_eq!(class, DeviceClass::UsbStorage);
        assert!(caps.contains(DeviceCapabilities::REMOVABLE));
    }

    #[test]
    fn test_enumerate_block_devices_nonexistent() {
        let devices = enumerate_block_devices(Path::new("/nonexistent/path"));
        assert!(devices.is_empty());
    }

    #[test]
    fn test_contains_bytes() {
        assert!(contains_bytes(b"/sys/block/dm-0", b"/dm-"));
        assert!(contains_bytes(b"/sys/block/loop0", b"/loop"));
        assert!(!contains_bytes(b"/sys/block/sda", b"/dm-"));
        assert!(!contains_bytes(b"", b"/dm-"));
    }
}
