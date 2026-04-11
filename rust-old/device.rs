//! Core device types, traits, and capabilities.

use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

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

impl From<&str> for DeviceId {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for DeviceId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// High-level device classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
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
#[non_exhaustive]
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
#[non_exhaustive]
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

/// Device node permissions (from stat()).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevicePermissions {
    pub uid: u32,
    pub gid: u32,
    pub mode: u32,
}

/// Query device node permissions via `std::fs::metadata()`.
#[cfg(target_os = "linux")]
pub fn query_permissions(dev_path: &Path) -> Option<DevicePermissions> {
    use std::os::unix::fs::MetadataExt;
    let meta = std::fs::metadata(dev_path).ok()?;
    Some(DevicePermissions {
        uid: meta.uid(),
        gid: meta.gid(),
        mode: meta.mode(),
    })
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
    /// Device node permissions (from stat()).
    pub permissions: Option<DevicePermissions>,
    /// USB vendor ID (e.g. 0x0781 for SanDisk).
    pub usb_vendor_id: Option<u16>,
    /// USB product ID.
    pub usb_product_id: Option<u16>,
    /// Partition table type (mbr, gpt).
    pub partition_table: Option<String>,
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
            permissions: None,
            usb_vendor_id: None,
            usb_product_id: None,
            partition_table: None,
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

/// Health information for a block device, read from sysfs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceHealth {
    /// Rotational (HDD) or non-rotational (SSD/NVMe).
    pub rotational: Option<bool>,
    /// NVMe temperature in Celsius (from hwmon).
    pub temperature_celsius: Option<f64>,
    /// Device scheduler.
    pub scheduler: Option<String>,
}

/// Query device health information from sysfs.
///
/// Reads:
/// - `/sys/block/<dev>/queue/rotational` (0=SSD, 1=HDD)
/// - `/sys/block/<dev>/device/hwmon/hwmon*/temp1_input` (millidegrees C for NVMe)
/// - `/sys/block/<dev>/queue/scheduler`
#[cfg(target_os = "linux")]
pub fn query_device_health(dev_name: &str) -> DeviceHealth {
    let base = PathBuf::from(format!("/sys/block/{}", dev_name));

    let rotational = std::fs::read_to_string(base.join("queue/rotational"))
        .ok()
        .and_then(|s| s.trim().parse::<u8>().ok())
        .map(|v| v == 1);

    let temperature_celsius = read_hwmon_temperature(&base);

    let scheduler = std::fs::read_to_string(base.join("queue/scheduler"))
        .ok()
        .map(|s| s.trim().to_string());

    DeviceHealth {
        rotational,
        temperature_celsius,
        scheduler,
    }
}

/// Search for hwmon temperature under the device's sysfs path.
#[cfg(target_os = "linux")]
fn read_hwmon_temperature(base: &std::path::Path) -> Option<f64> {
    let hwmon_dir = base.join("device/hwmon");
    let entries = std::fs::read_dir(&hwmon_dir).ok()?;
    for entry in entries.flatten() {
        let temp_path = entry.path().join("temp1_input");
        if let Ok(contents) = std::fs::read_to_string(&temp_path)
            && let Ok(millidegrees) = contents.trim().parse::<f64>()
        {
            return Some(millidegrees / 1000.0);
        }
    }
    None
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
    fn test_device_id_from_str() {
        let id: DeviceId = "block:sdb1".into();
        assert_eq!(id.as_str(), "block:sdb1");
    }

    #[test]
    fn test_device_id_from_string() {
        let id: DeviceId = String::from("block:sdc1").into();
        assert_eq!(id.as_str(), "block:sdc1");
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

    // -----------------------------------------------------------------------
    // DeviceHealth tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_device_health_serde_roundtrip() {
        let health = DeviceHealth {
            rotational: Some(true),
            temperature_celsius: Some(42.5),
            scheduler: Some("[mq-deadline] none".to_string()),
        };
        let json = serde_json::to_string(&health).unwrap();
        let roundtrip: DeviceHealth = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.rotational, Some(true));
        assert_eq!(roundtrip.temperature_celsius, Some(42.5));
        assert_eq!(roundtrip.scheduler.as_deref(), Some("[mq-deadline] none"));
    }

    #[test]
    fn test_device_health_all_none() {
        let health = DeviceHealth {
            rotational: None,
            temperature_celsius: None,
            scheduler: None,
        };
        let json = serde_json::to_string(&health).unwrap();
        let roundtrip: DeviceHealth = serde_json::from_str(&json).unwrap();
        assert!(roundtrip.rotational.is_none());
        assert!(roundtrip.temperature_celsius.is_none());
        assert!(roundtrip.scheduler.is_none());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_query_device_health_nonexistent() {
        let health = query_device_health("nonexistent_yukti_test_device");
        assert!(health.rotational.is_none());
        assert!(health.temperature_celsius.is_none());
        assert!(health.scheduler.is_none());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_query_device_health_fake_sysfs() {
        let dir = std::env::temp_dir().join("yukti_test_health_sysfs");
        let _ = std::fs::remove_dir_all(&dir);

        // We can't easily test query_device_health with a fake sysfs root
        // because it hardcodes /sys/block/. Instead, verify the function
        // handles missing paths gracefully.
        let health = query_device_health("yukti_fake_block_device_12345");
        assert!(health.rotational.is_none());
        assert!(health.temperature_celsius.is_none());
        assert!(health.scheduler.is_none());
    }

    #[test]
    fn test_device_permissions_serde() {
        let perms = DevicePermissions {
            uid: 0,
            gid: 6,
            mode: 0o060660,
        };
        let json = serde_json::to_string(&perms).unwrap();
        let roundtrip: DevicePermissions = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.uid, 0);
        assert_eq!(roundtrip.gid, 6);
        assert_eq!(roundtrip.mode, 0o060660);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_query_permissions_existing_file() {
        // /dev/null should always exist and be readable
        let perms = query_permissions(std::path::Path::new("/dev/null"));
        assert!(perms.is_some());
        let p = perms.unwrap();
        // /dev/null is owned by root (uid 0)
        assert_eq!(p.uid, 0);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_query_permissions_nonexistent() {
        let perms = query_permissions(std::path::Path::new("/dev/yukti_nonexistent_device_xyz"));
        assert!(perms.is_none());
    }

    #[test]
    fn test_device_info_new_fields_default() {
        let info = DeviceInfo::new(
            DeviceId::new("test"),
            PathBuf::from("/dev/sda"),
            DeviceClass::BlockInternal,
        );
        assert!(info.permissions.is_none());
        assert!(info.usb_vendor_id.is_none());
        assert!(info.usb_product_id.is_none());
        assert!(info.partition_table.is_none());
    }

    #[test]
    fn test_device_info_usb_ids() {
        let mut info = DeviceInfo::new(
            DeviceId::new("test"),
            PathBuf::from("/dev/sdb1"),
            DeviceClass::UsbStorage,
        );
        info.usb_vendor_id = Some(0x0781);
        info.usb_product_id = Some(0x5567);
        assert_eq!(info.usb_vendor_id, Some(0x0781));
        assert_eq!(info.usb_product_id, Some(0x5567));

        // Serde roundtrip
        let json = serde_json::to_string(&info).unwrap();
        let roundtrip: DeviceInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.usb_vendor_id, Some(0x0781));
        assert_eq!(roundtrip.usb_product_id, Some(0x5567));
    }

    #[test]
    fn test_device_info_partition_table() {
        let mut info = DeviceInfo::new(
            DeviceId::new("test"),
            PathBuf::from("/dev/sda"),
            DeviceClass::BlockInternal,
        );
        info.partition_table = Some("gpt".into());
        assert_eq!(info.partition_table.as_deref(), Some("gpt"));

        let json = serde_json::to_string(&info).unwrap();
        let roundtrip: DeviceInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.partition_table.as_deref(), Some("gpt"));
    }
}
