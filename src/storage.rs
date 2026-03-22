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
    /// Zero-allocation for known types (uses case-insensitive comparison).
    pub fn from_str_type(s: &str) -> Self {
        if s.eq_ignore_ascii_case("ext4") {
            Self::Ext4
        } else if s.eq_ignore_ascii_case("btrfs") {
            Self::Btrfs
        } else if s.eq_ignore_ascii_case("xfs") {
            Self::Xfs
        } else if s.eq_ignore_ascii_case("vfat")
            || s.eq_ignore_ascii_case("fat32")
            || s.eq_ignore_ascii_case("fat16")
        {
            Self::Vfat
        } else if s.eq_ignore_ascii_case("ntfs") {
            Self::Ntfs
        } else if s.eq_ignore_ascii_case("exfat") {
            Self::Exfat
        } else if s.eq_ignore_ascii_case("iso9660") {
            Self::Iso9660
        } else if s.eq_ignore_ascii_case("udf") {
            Self::Udf
        } else if s.eq_ignore_ascii_case("hfsplus") || s.eq_ignore_ascii_case("hfs+") {
            Self::Hfsplus
        } else if s.eq_ignore_ascii_case("swap") {
            Self::Swap
        } else if s.eq_ignore_ascii_case("crypto_luks") || s.eq_ignore_ascii_case("luks") {
            Self::Luks
        } else {
            Self::Unknown(s.to_string())
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
        let s = match self {
            Self::Ext4 => "ext4",
            Self::Btrfs => "btrfs",
            Self::Xfs => "xfs",
            Self::Vfat => "vfat",
            Self::Ntfs => "ntfs",
            Self::Exfat => "exfat",
            Self::Iso9660 => "iso9660",
            Self::Udf => "udf",
            Self::Hfsplus => "hfs+",
            Self::Swap => "swap",
            Self::Luks => "luks",
            Self::Unknown(s) => return f.write_str(s),
        };
        f.write_str(s)
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

/// Forbidden system mount points.
const FORBIDDEN_MOUNTS: &[&str] = &[
    "/", "/bin", "/sbin", "/usr", "/etc", "/boot", "/sys", "/proc", "/dev",
];

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
/// Compares paths directly (no string allocation).
pub fn validate_mount_point(path: &Path) -> Result<()> {
    // Must be absolute
    if !path.is_absolute() {
        return Err(YantraError::MountFailed {
            device: String::new(),
            reason: format!("mount point must be absolute: {}", path.display()),
        });
    }
    // Must not be a system directory
    for f in FORBIDDEN_MOUNTS {
        if path == Path::new(f) {
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
    fn test_filesystem_parse_all_known() {
        assert_eq!(Filesystem::from_str_type("ext4"), Filesystem::Ext4);
        assert_eq!(Filesystem::from_str_type("btrfs"), Filesystem::Btrfs);
        assert_eq!(Filesystem::from_str_type("xfs"), Filesystem::Xfs);
        assert_eq!(Filesystem::from_str_type("vfat"), Filesystem::Vfat);
        assert_eq!(Filesystem::from_str_type("fat16"), Filesystem::Vfat);
        assert_eq!(Filesystem::from_str_type("ntfs"), Filesystem::Ntfs);
        assert_eq!(Filesystem::from_str_type("exfat"), Filesystem::Exfat);
        assert_eq!(Filesystem::from_str_type("iso9660"), Filesystem::Iso9660);
        assert_eq!(Filesystem::from_str_type("udf"), Filesystem::Udf);
        assert_eq!(Filesystem::from_str_type("hfsplus"), Filesystem::Hfsplus);
        assert_eq!(Filesystem::from_str_type("hfs+"), Filesystem::Hfsplus);
        assert_eq!(Filesystem::from_str_type("swap"), Filesystem::Swap);
        assert_eq!(Filesystem::from_str_type("luks"), Filesystem::Luks);
        assert_eq!(Filesystem::from_str_type("crypto_luks"), Filesystem::Luks);
    }

    #[test]
    fn test_filesystem_parse_case_insensitive() {
        assert_eq!(Filesystem::from_str_type("EXT4"), Filesystem::Ext4);
        assert_eq!(Filesystem::from_str_type("Ext4"), Filesystem::Ext4);
        assert_eq!(Filesystem::from_str_type("NTFS"), Filesystem::Ntfs);
        assert_eq!(Filesystem::from_str_type("Btrfs"), Filesystem::Btrfs);
        assert_eq!(Filesystem::from_str_type("XFS"), Filesystem::Xfs);
        assert_eq!(Filesystem::from_str_type("ISO9660"), Filesystem::Iso9660);
        assert_eq!(Filesystem::from_str_type("UDF"), Filesystem::Udf);
    }

    #[test]
    fn test_filesystem_writable() {
        assert!(Filesystem::Ext4.is_writable());
        assert!(Filesystem::Btrfs.is_writable());
        assert!(Filesystem::Xfs.is_writable());
        assert!(Filesystem::Vfat.is_writable());
        assert!(Filesystem::Ntfs.is_writable());
        assert!(Filesystem::Exfat.is_writable());
        assert!(Filesystem::Hfsplus.is_writable());
        assert!(!Filesystem::Iso9660.is_writable());
        assert!(!Filesystem::Udf.is_writable());
        assert!(!Filesystem::Swap.is_writable());
        assert!(!Filesystem::Luks.is_writable());
        assert!(!Filesystem::Unknown("zfs".into()).is_writable());
    }

    #[test]
    fn test_filesystem_optical() {
        assert!(Filesystem::Iso9660.is_optical_media());
        assert!(Filesystem::Udf.is_optical_media());
        assert!(!Filesystem::Ext4.is_optical_media());
        assert!(!Filesystem::Vfat.is_optical_media());
    }

    #[test]
    fn test_filesystem_display_all() {
        assert_eq!(Filesystem::Ext4.to_string(), "ext4");
        assert_eq!(Filesystem::Btrfs.to_string(), "btrfs");
        assert_eq!(Filesystem::Xfs.to_string(), "xfs");
        assert_eq!(Filesystem::Vfat.to_string(), "vfat");
        assert_eq!(Filesystem::Ntfs.to_string(), "ntfs");
        assert_eq!(Filesystem::Exfat.to_string(), "exfat");
        assert_eq!(Filesystem::Iso9660.to_string(), "iso9660");
        assert_eq!(Filesystem::Udf.to_string(), "udf");
        assert_eq!(Filesystem::Hfsplus.to_string(), "hfs+");
        assert_eq!(Filesystem::Swap.to_string(), "swap");
        assert_eq!(Filesystem::Luks.to_string(), "luks");
        assert_eq!(Filesystem::Unknown("zfs".into()).to_string(), "zfs");
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
    fn test_default_mount_point_no_label() {
        let info = DeviceInfo::new(
            DeviceId::new("block:sdb1"),
            PathBuf::from("/dev/sdb1"),
            DeviceClass::UsbStorage,
        );
        let mp = default_mount_point(&info);
        assert_eq!(mp, PathBuf::from("/run/media/block_sdb1"));
    }

    #[test]
    fn test_validate_mount_point_ok() {
        assert!(validate_mount_point(Path::new("/mnt/usb")).is_ok());
        assert!(validate_mount_point(Path::new("/run/media/disk")).is_ok());
        assert!(validate_mount_point(Path::new("/home/user/mnt")).is_ok());
    }

    #[test]
    fn test_validate_mount_point_forbidden() {
        assert!(validate_mount_point(Path::new("/")).is_err());
        assert!(validate_mount_point(Path::new("/usr")).is_err());
        assert!(validate_mount_point(Path::new("/boot")).is_err());
        assert!(validate_mount_point(Path::new("/dev")).is_err());
        assert!(validate_mount_point(Path::new("/sys")).is_err());
        assert!(validate_mount_point(Path::new("/proc")).is_err());
    }

    #[test]
    fn test_validate_mount_point_relative() {
        assert!(validate_mount_point(Path::new("mnt/usb")).is_err());
    }

    #[test]
    fn test_is_mounted_stub() {
        assert!(!is_mounted(Path::new("/dev/sdb1")));
    }

    #[test]
    fn test_find_mount_point_stub() {
        assert!(find_mount_point(Path::new("/dev/sdb1")).is_none());
    }

    #[test]
    fn test_mount_result_serde() {
        let result = MountResult {
            device_id: DeviceId::new("block:sdb1"),
            dev_path: PathBuf::from("/dev/sdb1"),
            mount_point: PathBuf::from("/mnt/usb"),
            fs_type: Filesystem::Vfat,
            read_only: false,
        };
        let json = serde_json::to_string(&result).unwrap();
        let roundtrip: MountResult = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.device_id, result.device_id);
        assert_eq!(roundtrip.fs_type, result.fs_type);
    }
}
