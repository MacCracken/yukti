//! USB/block device storage operations — mount, unmount, eject, filesystem detection.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::device::{DeviceId, DeviceInfo};
use crate::error::{Result, YantraError};

/// Filesystem type detected from a device.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Filesystem {
    Ext4,
    Btrfs,
    Xfs,
    Vfat,
    Ntfs,
    Exfat,
    Iso9660,
    Udf,
    Hfsplus,
    Swap,
    Luks,
    Unknown(String),
}

impl Filesystem {
    /// Parse from a filesystem type string (as returned by blkid/lsblk).
    pub fn from_str_type(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "ext4" => Self::Ext4,
            "btrfs" => Self::Btrfs,
            "xfs" => Self::Xfs,
            "vfat" | "fat32" | "fat16" => Self::Vfat,
            "ntfs" => Self::Ntfs,
            "exfat" => Self::Exfat,
            "iso9660" => Self::Iso9660,
            "udf" => Self::Udf,
            "hfsplus" | "hfs+" => Self::Hfsplus,
            "swap" => Self::Swap,
            "crypto_luks" | "luks" => Self::Luks,
            other => Self::Unknown(other.to_string()),
        }
    }

    /// Whether this filesystem is read-write capable.
    pub fn is_writable(&self) -> bool {
        matches!(
            self,
            Self::Ext4
                | Self::Btrfs
                | Self::Xfs
                | Self::Vfat
                | Self::Ntfs
                | Self::Exfat
                | Self::Hfsplus
        )
    }

    /// Whether this filesystem is typically read-only (optical media).
    pub fn is_optical_media(&self) -> bool {
        matches!(self, Self::Iso9660 | Self::Udf)
    }
}

impl std::fmt::Display for Filesystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ext4 => write!(f, "ext4"),
            Self::Btrfs => write!(f, "btrfs"),
            Self::Xfs => write!(f, "xfs"),
            Self::Vfat => write!(f, "vfat"),
            Self::Ntfs => write!(f, "ntfs"),
            Self::Exfat => write!(f, "exfat"),
            Self::Iso9660 => write!(f, "iso9660"),
            Self::Udf => write!(f, "udf"),
            Self::Hfsplus => write!(f, "hfs+"),
            Self::Swap => write!(f, "swap"),
            Self::Luks => write!(f, "luks"),
            Self::Unknown(s) => write!(f, "{s}"),
        }
    }
}

/// Mount options for a device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountOptions {
    /// Mount point path. If None, auto-select under /run/media/$USER/ or /mnt/.
    pub mount_point: Option<PathBuf>,
    /// Mount read-only.
    pub read_only: bool,
    /// Extra mount options (e.g. "noexec", "nosuid").
    pub options: Vec<String>,
    /// Filesystem type override (auto-detect if None).
    pub fs_type: Option<String>,
}

impl Default for MountOptions {
    fn default() -> Self {
        Self {
            mount_point: None,
            read_only: false,
            options: vec!["nosuid".into(), "nodev".into()],
            fs_type: None,
        }
    }
}

/// Result of a mount operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountResult {
    pub device_id: DeviceId,
    pub dev_path: PathBuf,
    pub mount_point: PathBuf,
    pub fs_type: Filesystem,
    pub read_only: bool,
}

/// Parse /proc/mounts to find mount points.
pub fn find_mount_point(dev_path: &Path) -> Option<PathBuf> {
    // In production: read /proc/mounts and match dev_path
    // For now: return None (not mounted)
    let _ = dev_path;
    None
}

/// Generate a default mount point for a device.
pub fn default_mount_point(device: &DeviceInfo) -> PathBuf {
    let name = device
        .label
        .as_deref()
        .unwrap_or_else(|| device.id.as_str());
    // Sanitize: replace non-alphanumeric with underscore
    let safe_name: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    PathBuf::from(format!("/run/media/{safe_name}"))
}

/// Check if a device is currently mounted by reading /proc/mounts.
pub fn is_mounted(dev_path: &Path) -> bool {
    find_mount_point(dev_path).is_some()
}

/// Validate that a mount point is safe to use.
pub fn validate_mount_point(path: &Path) -> Result<()> {
    // Must be absolute
    if !path.is_absolute() {
        return Err(YantraError::MountFailed {
            device: String::new(),
            reason: format!("mount point must be absolute: {}", path.display()),
        });
    }
    // Must not be a system directory
    let forbidden = ["/", "/bin", "/sbin", "/usr", "/etc", "/boot", "/sys", "/proc", "/dev"];
    let path_str = path.to_string_lossy();
    for f in &forbidden {
        if path_str == *f {
            return Err(YantraError::MountFailed {
                device: String::new(),
                reason: format!("cannot mount on system directory: {f}"),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::{DeviceClass, DeviceId};

    #[test]
    fn test_filesystem_parse() {
        assert_eq!(Filesystem::from_str_type("ext4"), Filesystem::Ext4);
        assert_eq!(Filesystem::from_str_type("VFAT"), Filesystem::Vfat);
        assert_eq!(Filesystem::from_str_type("fat32"), Filesystem::Vfat);
        assert_eq!(Filesystem::from_str_type("iso9660"), Filesystem::Iso9660);
        assert_eq!(Filesystem::from_str_type("crypto_luks"), Filesystem::Luks);
        assert!(matches!(
            Filesystem::from_str_type("zfs"),
            Filesystem::Unknown(_)
        ));
    }

    #[test]
    fn test_filesystem_writable() {
        assert!(Filesystem::Ext4.is_writable());
        assert!(Filesystem::Vfat.is_writable());
        assert!(!Filesystem::Iso9660.is_writable());
        assert!(!Filesystem::Swap.is_writable());
    }

    #[test]
    fn test_filesystem_optical() {
        assert!(Filesystem::Iso9660.is_optical_media());
        assert!(Filesystem::Udf.is_optical_media());
        assert!(!Filesystem::Ext4.is_optical_media());
    }

    #[test]
    fn test_mount_options_default() {
        let opts = MountOptions::default();
        assert!(!opts.read_only);
        assert!(opts.options.contains(&"nosuid".to_string()));
        assert!(opts.options.contains(&"nodev".to_string()));
    }

    #[test]
    fn test_default_mount_point() {
        let mut info = DeviceInfo::new(
            DeviceId::new("block:sdb1"),
            PathBuf::from("/dev/sdb1"),
            DeviceClass::UsbStorage,
        );
        info.label = Some("MY USB".into());
        let mp = default_mount_point(&info);
        assert_eq!(mp, PathBuf::from("/run/media/MY_USB"));
    }

    #[test]
    fn test_validate_mount_point_ok() {
        assert!(validate_mount_point(Path::new("/mnt/usb")).is_ok());
        assert!(validate_mount_point(Path::new("/run/media/disk")).is_ok());
    }

    #[test]
    fn test_validate_mount_point_forbidden() {
        assert!(validate_mount_point(Path::new("/")).is_err());
        assert!(validate_mount_point(Path::new("/usr")).is_err());
        assert!(validate_mount_point(Path::new("/boot")).is_err());
    }

    #[test]
    fn test_validate_mount_point_relative() {
        assert!(validate_mount_point(Path::new("mnt/usb")).is_err());
    }

    #[test]
    fn test_filesystem_display() {
        assert_eq!(Filesystem::Ext4.to_string(), "ext4");
        assert_eq!(Filesystem::Vfat.to_string(), "vfat");
        assert_eq!(Filesystem::Unknown("zfs".into()).to_string(), "zfs");
    }
}
