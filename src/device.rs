//! Core device types, traits, and capabilities.

use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;

use bitflags::bitflags;
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
        f.write_str(&self.0)
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
        let s = match self {
            Self::UsbStorage => "usb-storage",
            Self::Optical => "optical",
            Self::BlockInternal => "block-internal",
            Self::SdCard => "sd-card",
            Self::Network => "network",
            Self::Loop => "loop",
            Self::DeviceMapper => "device-mapper",
            Self::Unknown => "unknown",
        };
        f.write_str(s)
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

impl DeviceCapability {
    /// Convert to the corresponding bitflag.
    #[inline]
    pub fn flag(self) -> DeviceCapabilities {
        match self {
            Self::Read => DeviceCapabilities::READ,
            Self::Write => DeviceCapabilities::WRITE,
            Self::Eject => DeviceCapabilities::EJECT,
            Self::MediaChange => DeviceCapabilities::MEDIA_CHANGE,
            Self::Removable => DeviceCapabilities::REMOVABLE,
            Self::Trim => DeviceCapabilities::TRIM,
            Self::Hotplug => DeviceCapabilities::HOTPLUG,
            Self::TrayControl => DeviceCapabilities::TRAY_CONTROL,
            Self::AudioPlayback => DeviceCapabilities::AUDIO_PLAYBACK,
            Self::Burn => DeviceCapabilities::BURN,
        }
    }
}

bitflags! {
    /// Bitflag storage for device capabilities — O(1) membership checks.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct DeviceCapabilities: u16 {
        const READ           = 1 << 0;
        const WRITE          = 1 << 1;
        const EJECT          = 1 << 2;
        const MEDIA_CHANGE   = 1 << 3;
        const REMOVABLE      = 1 << 4;
        const TRIM           = 1 << 5;
        const HOTPLUG        = 1 << 6;
        const TRAY_CONTROL   = 1 << 7;
        const AUDIO_PLAYBACK = 1 << 8;
        const BURN           = 1 << 9;
    }
}

/// All individual capability variants for iteration.
const ALL_CAPABILITIES: &[(DeviceCapabilities, DeviceCapability)] = &[
    (DeviceCapabilities::READ, DeviceCapability::Read),
    (DeviceCapabilities::WRITE, DeviceCapability::Write),
    (DeviceCapabilities::EJECT, DeviceCapability::Eject),
    (
        DeviceCapabilities::MEDIA_CHANGE,
        DeviceCapability::MediaChange,
    ),
    (DeviceCapabilities::REMOVABLE, DeviceCapability::Removable),
    (DeviceCapabilities::TRIM, DeviceCapability::Trim),
    (DeviceCapabilities::HOTPLUG, DeviceCapability::Hotplug),
    (
        DeviceCapabilities::TRAY_CONTROL,
        DeviceCapability::TrayControl,
    ),
    (
        DeviceCapabilities::AUDIO_PLAYBACK,
        DeviceCapability::AudioPlayback,
    ),
    (DeviceCapabilities::BURN, DeviceCapability::Burn),
];

impl DeviceCapabilities {
    /// Convert to a Vec of individual capabilities (for display/serde).
    pub fn to_vec(self) -> Vec<DeviceCapability> {
        ALL_CAPABILITIES
            .iter()
            .filter(|(flag, _)| self.contains(*flag))
            .map(|(_, cap)| *cap)
            .collect()
    }

    /// Build from a slice of capabilities.
    pub fn from_slice(caps: &[DeviceCapability]) -> Self {
        let mut flags = Self::empty();
        for cap in caps {
            flags |= cap.flag();
        }
        flags
    }
}

impl Serialize for DeviceCapabilities {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.to_vec().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for DeviceCapabilities {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let caps: Vec<DeviceCapability> = Vec::deserialize(deserializer)?;
        Ok(Self::from_slice(&caps))
    }
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
        let s = match self {
            Self::Ready => "ready",
            Self::Mounted => "mounted",
            Self::NoMedia => "no-media",
            Self::Ejecting => "ejecting",
            Self::Detached => "detached",
            Self::Error => "error",
        };
        f.write_str(s)
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
    /// Device capabilities (bitflag — serializes as array for compatibility).
    pub capabilities: DeviceCapabilities,
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
            capabilities: DeviceCapabilities::empty(),
            detected_at: Utc::now(),
            properties: HashMap::new(),
        }
    }

    /// Check if the device has a specific capability. O(1) bitwise check.
    #[inline]
    pub fn has_capability(&self, cap: DeviceCapability) -> bool {
        self.capabilities.contains(cap.flag())
    }

    /// Check if the device is currently mounted.
    #[inline]
    pub fn is_mounted(&self) -> bool {
        self.state == DeviceState::Mounted || self.mount_point.is_some()
    }

    /// Check if the device is removable (USB, optical, SD card).
    #[inline]
    pub fn is_removable(&self) -> bool {
        matches!(
            self.class,
            DeviceClass::UsbStorage | DeviceClass::Optical | DeviceClass::SdCard
        ) || self.capabilities.contains(DeviceCapabilities::REMOVABLE)
    }

    /// Human-readable display name: label > model > dev_path.
    pub fn display_name(&self) -> Cow<'_, str> {
        if let Some(label) = &self.label {
            Cow::Borrowed(label)
        } else if let Some(model) = &self.model {
            Cow::Borrowed(model)
        } else {
            Cow::Owned(self.dev_path.display().to_string())
        }
    }

    /// Size formatted as human-readable string.
    pub fn size_display(&self) -> Cow<'static, str> {
        let bytes = self.size_bytes;
        if bytes == 0 {
            return Cow::Borrowed("unknown");
        }
        const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
        let mut size = bytes as f64;
        let mut unit_idx = 0;
        while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
            size /= 1024.0;
            unit_idx += 1;
        }
        if unit_idx == 0 {
            Cow::Owned(format!("{bytes} B"))
        } else {
            Cow::Owned(format!("{size:.1} {}", UNITS[unit_idx]))
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
    fn test_device_id_equality_and_hash() {
        use std::collections::HashSet;
        let a = DeviceId::new("block:sdb1");
        let b = DeviceId::new("block:sdb1");
        let c = DeviceId::new("block:sdc1");
        assert_eq!(a, b);
        assert_ne!(a, c);
        let mut set = HashSet::new();
        set.insert(a.clone());
        assert!(set.contains(&b));
        assert!(!set.contains(&c));
    }

    #[test]
    fn test_device_class_display_all() {
        assert_eq!(DeviceClass::UsbStorage.to_string(), "usb-storage");
        assert_eq!(DeviceClass::Optical.to_string(), "optical");
        assert_eq!(DeviceClass::BlockInternal.to_string(), "block-internal");
        assert_eq!(DeviceClass::SdCard.to_string(), "sd-card");
        assert_eq!(DeviceClass::Network.to_string(), "network");
        assert_eq!(DeviceClass::Loop.to_string(), "loop");
        assert_eq!(DeviceClass::DeviceMapper.to_string(), "device-mapper");
        assert_eq!(DeviceClass::Unknown.to_string(), "unknown");
    }

    #[test]
    fn test_device_state_display_all() {
        assert_eq!(DeviceState::Ready.to_string(), "ready");
        assert_eq!(DeviceState::Mounted.to_string(), "mounted");
        assert_eq!(DeviceState::NoMedia.to_string(), "no-media");
        assert_eq!(DeviceState::Ejecting.to_string(), "ejecting");
        assert_eq!(DeviceState::Detached.to_string(), "detached");
        assert_eq!(DeviceState::Error.to_string(), "error");
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
        assert!(info.capabilities.is_empty());
    }

    #[test]
    fn test_has_capability_bitflags() {
        let mut info = DeviceInfo::new(
            DeviceId::new("block:sr0"),
            PathBuf::from("/dev/sr0"),
            DeviceClass::Optical,
        );
        assert!(!info.has_capability(DeviceCapability::Eject));
        info.capabilities |= DeviceCapability::Eject.flag();
        info.capabilities |= DeviceCapability::TrayControl.flag();
        assert!(info.has_capability(DeviceCapability::Eject));
        assert!(info.has_capability(DeviceCapability::TrayControl));
        assert!(!info.has_capability(DeviceCapability::Burn));
    }

    #[test]
    fn test_capabilities_to_vec_roundtrip() {
        let caps =
            DeviceCapabilities::READ | DeviceCapabilities::WRITE | DeviceCapabilities::REMOVABLE;
        let vec = caps.to_vec();
        assert_eq!(vec.len(), 3);
        assert!(vec.contains(&DeviceCapability::Read));
        assert!(vec.contains(&DeviceCapability::Write));
        assert!(vec.contains(&DeviceCapability::Removable));

        let rebuilt = DeviceCapabilities::from_slice(&vec);
        assert_eq!(caps, rebuilt);
    }

    #[test]
    fn test_capabilities_empty() {
        let caps = DeviceCapabilities::empty();
        assert!(caps.to_vec().is_empty());
        assert!(!caps.contains(DeviceCapabilities::READ));
    }

    #[test]
    fn test_capabilities_all() {
        let caps = DeviceCapabilities::from_slice(&[
            DeviceCapability::Read,
            DeviceCapability::Write,
            DeviceCapability::Eject,
            DeviceCapability::MediaChange,
            DeviceCapability::Removable,
            DeviceCapability::Trim,
            DeviceCapability::Hotplug,
            DeviceCapability::TrayControl,
            DeviceCapability::AudioPlayback,
            DeviceCapability::Burn,
        ]);
        assert_eq!(caps.to_vec().len(), 10);
    }

    #[test]
    fn test_display_name_priority() {
        let mut info = DeviceInfo::new(
            DeviceId::new("test"),
            PathBuf::from("/dev/sdc1"),
            DeviceClass::UsbStorage,
        );
        assert_eq!(info.display_name().as_ref(), "/dev/sdc1");

        info.model = Some("SanDisk Cruzer".into());
        assert_eq!(info.display_name().as_ref(), "SanDisk Cruzer");

        info.label = Some("MY_USB".into());
        assert_eq!(info.display_name().as_ref(), "MY_USB");
    }

    #[test]
    fn test_size_display() {
        let mut info = DeviceInfo::new(
            DeviceId::new("test"),
            PathBuf::from("/dev/sda"),
            DeviceClass::BlockInternal,
        );
        assert_eq!(info.size_display().as_ref(), "unknown");

        info.size_bytes = 512;
        assert_eq!(info.size_display().as_ref(), "512 B");

        info.size_bytes = 1024;
        assert_eq!(info.size_display().as_ref(), "1.0 KB");

        info.size_bytes = 1024 * 1024 * 1024 * 16; // 16 GB
        assert!(info.size_display().contains("16.0 GB"));
    }

    #[test]
    fn test_size_display_tb() {
        let mut info = DeviceInfo::new(
            DeviceId::new("test"),
            PathBuf::from("/dev/sda"),
            DeviceClass::BlockInternal,
        );
        info.size_bytes = 1024 * 1024 * 1024 * 1024 * 2; // 2 TB
        assert!(info.size_display().contains("2.0 TB"));
    }

    #[test]
    fn test_is_removable() {
        let usb = DeviceInfo::new(
            DeviceId::new("usb"),
            PathBuf::from("/dev/sdb1"),
            DeviceClass::UsbStorage,
        );
        assert!(usb.is_removable());

        let sd = DeviceInfo::new(
            DeviceId::new("sd"),
            PathBuf::from("/dev/mmcblk0"),
            DeviceClass::SdCard,
        );
        assert!(sd.is_removable());

        let internal = DeviceInfo::new(
            DeviceId::new("ssd"),
            PathBuf::from("/dev/nvme0n1"),
            DeviceClass::BlockInternal,
        );
        assert!(!internal.is_removable());

        // BlockInternal with Removable capability
        let mut hotplug_internal = DeviceInfo::new(
            DeviceId::new("hotplug"),
            PathBuf::from("/dev/sda"),
            DeviceClass::BlockInternal,
        );
        hotplug_internal.capabilities |= DeviceCapabilities::REMOVABLE;
        assert!(hotplug_internal.is_removable());
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
    fn test_is_mounted_by_mount_point_only() {
        let mut info = DeviceInfo::new(
            DeviceId::new("test"),
            PathBuf::from("/dev/sdb1"),
            DeviceClass::UsbStorage,
        );
        // mount_point set but state is Ready
        info.mount_point = Some(PathBuf::from("/mnt/usb"));
        assert!(info.is_mounted());
    }

    #[test]
    fn test_serialization_roundtrip() {
        let mut info = DeviceInfo::new(
            DeviceId::new("test:dev"),
            PathBuf::from("/dev/sdb"),
            DeviceClass::UsbStorage,
        );
        info.capabilities =
            DeviceCapabilities::READ | DeviceCapabilities::WRITE | DeviceCapabilities::EJECT;
        info.label = Some("TEST_DRIVE".into());
        info.vendor = Some("TestVendor".into());
        info.size_bytes = 1024 * 1024;

        let json = serde_json::to_string(&info).unwrap();
        let roundtrip: DeviceInfo = serde_json::from_str(&json).unwrap();

        assert_eq!(roundtrip.id, info.id);
        assert_eq!(roundtrip.class, info.class);
        assert_eq!(roundtrip.capabilities, info.capabilities);
        assert_eq!(roundtrip.label, info.label);
        assert_eq!(roundtrip.size_bytes, info.size_bytes);
    }

    #[test]
    fn test_capabilities_serde() {
        let caps = DeviceCapabilities::READ | DeviceCapabilities::EJECT;
        let json = serde_json::to_string(&caps).unwrap();
        assert!(json.contains("Read"));
        assert!(json.contains("Eject"));
        let deserialized: DeviceCapabilities = serde_json::from_str(&json).unwrap();
        assert_eq!(caps, deserialized);
    }
}
