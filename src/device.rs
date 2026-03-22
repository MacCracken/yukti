//! Core device types, traits, and capabilities.

use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Unique device identifier (subsystem + sysname).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeviceId(pub String);

impl DeviceId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// High-level device classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DeviceClass {
    /// USB mass storage (flash drives, external HDDs).
    UsbStorage,
    /// Optical drive (CD/DVD/Blu-ray).
    Optical,
    /// Internal block device (HDD, SSD, NVMe).
    BlockInternal,
    /// SD/MMC card reader.
    SdCard,
    /// Network-attached storage.
    Network,
    /// Loop device.
    Loop,
    /// Device mapper (LVM, LUKS, dm-verity).
    DeviceMapper,
    /// Unknown or unclassified.
    Unknown,
}

impl fmt::Display for DeviceClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UsbStorage => write!(f, "usb-storage"),
            Self::Optical => write!(f, "optical"),
            Self::BlockInternal => write!(f, "block-internal"),
            Self::SdCard => write!(f, "sd-card"),
            Self::Network => write!(f, "network"),
            Self::Loop => write!(f, "loop"),
            Self::DeviceMapper => write!(f, "device-mapper"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// What a device can do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DeviceCapability {
    /// Can be read.
    Read,
    /// Can be written.
    Write,
    /// Can be ejected (USB safe-remove, optical tray).
    Eject,
    /// Supports media change detection.
    MediaChange,
    /// Has removable media (optical, floppy, USB).
    Removable,
    /// Supports TRIM/discard (SSD).
    Trim,
    /// Hotpluggable.
    Hotplug,
    /// Supports tray open/close (optical).
    TrayControl,
    /// Can play audio CDs directly.
    AudioPlayback,
    /// Can burn media.
    Burn,
}

/// Current state of a device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DeviceState {
    /// Device present, media loaded, not mounted.
    Ready,
    /// Device present, mounted at a path.
    Mounted,
    /// Device present, no media (empty optical drive).
    NoMedia,
    /// Device is being ejected.
    Ejecting,
    /// Device was removed (hotplug detach).
    Detached,
    /// Device error.
    Error,
}

impl fmt::Display for DeviceState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ready => write!(f, "ready"),
            Self::Mounted => write!(f, "mounted"),
            Self::NoMedia => write!(f, "no-media"),
            Self::Ejecting => write!(f, "ejecting"),
            Self::Detached => write!(f, "detached"),
            Self::Error => write!(f, "error"),
        }
    }
}

/// Information about a detected device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    /// Unique identifier.
    pub id: DeviceId,
    /// Device node path (e.g. /dev/sdb1, /dev/sr0).
    pub dev_path: PathBuf,
    /// Sysfs path.
    pub sys_path: Option<PathBuf>,
    /// Device class.
    pub class: DeviceClass,
    /// Current state.
    pub state: DeviceState,
    /// Human-readable label (partition label, disc title, USB product name).
    pub label: Option<String>,
    /// Vendor name.
    pub vendor: Option<String>,
    /// Model name.
    pub model: Option<String>,
    /// Serial number.
    pub serial: Option<String>,
    /// Filesystem type (ext4, vfat, iso9660, udf, etc.).
    pub fs_type: Option<String>,
    /// Mount point if mounted.
    pub mount_point: Option<PathBuf>,
    /// Size in bytes (0 if unknown or no media).
    pub size_bytes: u64,
    /// Device capabilities.
    pub capabilities: Vec<DeviceCapability>,
    /// When this device was first detected.
    pub detected_at: DateTime<Utc>,
    /// Extra properties from udev.
    pub properties: HashMap<String, String>,
}

impl DeviceInfo {
    /// Create a new DeviceInfo with minimal required fields.
    pub fn new(id: DeviceId, dev_path: PathBuf, class: DeviceClass) -> Self {
        Self {
            id,
            dev_path,
            sys_path: None,
            class,
            state: DeviceState::Ready,
            label: None,
            vendor: None,
            model: None,
            serial: None,
            fs_type: None,
            mount_point: None,
            size_bytes: 0,
            capabilities: Vec::new(),
            detected_at: Utc::now(),
            properties: HashMap::new(),
        }
    }

    /// Check if the device has a specific capability.
    pub fn has_capability(&self, cap: DeviceCapability) -> bool {
        self.capabilities.contains(&cap)
    }

    /// Check if the device is currently mounted.
    pub fn is_mounted(&self) -> bool {
        self.state == DeviceState::Mounted || self.mount_point.is_some()
    }

    /// Check if the device is removable (USB, optical, SD card).
    pub fn is_removable(&self) -> bool {
        matches!(
            self.class,
            DeviceClass::UsbStorage | DeviceClass::Optical | DeviceClass::SdCard
        ) || self.has_capability(DeviceCapability::Removable)
    }

    /// Human-readable display name: label > model > dev_path.
    pub fn display_name(&self) -> String {
        if let Some(label) = &self.label {
            label.clone()
        } else if let Some(model) = &self.model {
            model.clone()
        } else {
            self.dev_path.display().to_string()
        }
    }

    /// Size formatted as human-readable string.
    pub fn size_display(&self) -> String {
        let bytes = self.size_bytes;
        if bytes == 0 {
            return "unknown".into();
        }
        const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
        let mut size = bytes as f64;
        let mut unit_idx = 0;
        while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
            size /= 1024.0;
            unit_idx += 1;
        }
        if unit_idx == 0 {
            format!("{bytes} B")
        } else {
            format!("{size:.1} {}", UNITS[unit_idx])
        }
    }
}

/// Trait for device management backends.
pub trait Device: Send + Sync {
    /// Enumerate all currently connected devices.
    fn enumerate(&self) -> crate::error::Result<Vec<DeviceInfo>>;

    /// Get info for a specific device by ID.
    fn get(&self, id: &DeviceId) -> crate::error::Result<Option<DeviceInfo>>;

    /// Refresh device state (re-read from sysfs/udev).
    fn refresh(&self, id: &DeviceId) -> crate::error::Result<DeviceInfo>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_id() {
        let id = DeviceId::new("block:sdb1");
        assert_eq!(id.as_str(), "block:sdb1");
        assert_eq!(id.to_string(), "block:sdb1");
    }

    #[test]
    fn test_device_class_display() {
        assert_eq!(DeviceClass::UsbStorage.to_string(), "usb-storage");
        assert_eq!(DeviceClass::Optical.to_string(), "optical");
        assert_eq!(DeviceClass::BlockInternal.to_string(), "block-internal");
    }

    #[test]
    fn test_device_info_new() {
        let info = DeviceInfo::new(
            DeviceId::new("block:sdb1"),
            PathBuf::from("/dev/sdb1"),
            DeviceClass::UsbStorage,
        );
        assert_eq!(info.class, DeviceClass::UsbStorage);
        assert_eq!(info.state, DeviceState::Ready);
        assert!(!info.is_mounted());
        assert!(info.is_removable());
    }

    #[test]
    fn test_has_capability() {
        let mut info = DeviceInfo::new(
            DeviceId::new("block:sr0"),
            PathBuf::from("/dev/sr0"),
            DeviceClass::Optical,
        );
        assert!(!info.has_capability(DeviceCapability::Eject));
        info.capabilities.push(DeviceCapability::Eject);
        info.capabilities.push(DeviceCapability::TrayControl);
        assert!(info.has_capability(DeviceCapability::Eject));
        assert!(info.has_capability(DeviceCapability::TrayControl));
    }

    #[test]
    fn test_display_name_priority() {
        let mut info = DeviceInfo::new(
            DeviceId::new("test"),
            PathBuf::from("/dev/sdc1"),
            DeviceClass::UsbStorage,
        );
        assert_eq!(info.display_name(), "/dev/sdc1");

        info.model = Some("SanDisk Cruzer".into());
        assert_eq!(info.display_name(), "SanDisk Cruzer");

        info.label = Some("MY_USB".into());
        assert_eq!(info.display_name(), "MY_USB");
    }

    #[test]
    fn test_size_display() {
        let mut info = DeviceInfo::new(
            DeviceId::new("test"),
            PathBuf::from("/dev/sda"),
            DeviceClass::BlockInternal,
        );
        assert_eq!(info.size_display(), "unknown");

        info.size_bytes = 512;
        assert_eq!(info.size_display(), "512 B");

        info.size_bytes = 1024 * 1024 * 1024 * 16; // 16 GB
        assert!(info.size_display().contains("16.0 GB"));
    }

    #[test]
    fn test_is_removable() {
        let usb = DeviceInfo::new(
            DeviceId::new("usb"),
            PathBuf::from("/dev/sdb1"),
            DeviceClass::UsbStorage,
        );
        assert!(usb.is_removable());

        let internal = DeviceInfo::new(
            DeviceId::new("ssd"),
            PathBuf::from("/dev/nvme0n1"),
            DeviceClass::BlockInternal,
        );
        assert!(!internal.is_removable());
    }

    #[test]
    fn test_is_mounted() {
        let mut info = DeviceInfo::new(
            DeviceId::new("test"),
            PathBuf::from("/dev/sdb1"),
            DeviceClass::UsbStorage,
        );
        assert!(!info.is_mounted());

        info.state = DeviceState::Mounted;
        info.mount_point = Some(PathBuf::from("/mnt/usb"));
        assert!(info.is_mounted());
    }

    #[test]
    fn test_serialization() {
        let info = DeviceInfo::new(
            DeviceId::new("test:dev"),
            PathBuf::from("/dev/sdb"),
            DeviceClass::UsbStorage,
        );
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("usb-storage") || json.contains("UsbStorage"));
        let _roundtrip: DeviceInfo = serde_json::from_str(&json).unwrap();
    }
}
