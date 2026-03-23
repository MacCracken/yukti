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

/// Unescape octal escapes in mount paths from /proc/mounts.
///
/// The kernel encodes spaces as `\040`, tabs as `\011`, newlines as `\012`,
/// and backslashes as `\134`.
fn unescape_mount_path(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 3 < bytes.len() {
            let d1 = bytes[i + 1];
            let d2 = bytes[i + 2];
            let d3 = bytes[i + 3];
            if (b'0'..=b'7').contains(&d1)
                && (b'0'..=b'7').contains(&d2)
                && (b'0'..=b'7').contains(&d3)
            {
                let val =
                    (d1 - b'0') as u32 * 64 + (d2 - b'0') as u32 * 8 + (d3 - b'0') as u32;
                if let Some(ch) = char::from_u32(val) {
                    result.push(ch);
                } else {
                    // Invalid octal value, keep literal
                    result.push('\\');
                    result.push(d1 as char);
                    result.push(d2 as char);
                    result.push(d3 as char);
                }
                i += 4;
                continue;
            }
        }
        result.push(bytes[i] as char);
        i += 1;
    }
    result
}

/// Internal helper: find mount point for a device given mount table contents.
///
/// This is separated from `find_mount_point()` so unit tests can pass mock data.
fn find_mount_in(dev_path: &Path, mount_table: &str) -> Option<PathBuf> {
    // Try to canonicalize the dev_path to resolve symlinks.
    let canonical = std::fs::canonicalize(dev_path).unwrap_or_else(|_| dev_path.to_path_buf());

    for line in mount_table.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // Format: device mountpoint fstype options dump pass
        let mut fields = line.split_whitespace();
        let Some(device_field) = fields.next() else {
            continue;
        };
        let Some(mountpoint_field) = fields.next() else {
            continue;
        };

        let device = unescape_mount_path(device_field);
        let mountpoint = unescape_mount_path(mountpoint_field);

        let device_path = Path::new(&device);
        // Try to canonicalize the device from the mount table too.
        let device_canonical =
            std::fs::canonicalize(device_path).unwrap_or_else(|_| device_path.to_path_buf());

        if device_canonical == canonical {
            return Some(PathBuf::from(mountpoint));
        }
    }
    None
}

/// Parse /proc/mounts to find where a device is mounted.
pub fn find_mount_point(dev_path: &Path) -> Option<PathBuf> {
    let mount_table = std::fs::read_to_string("/proc/mounts").ok()?;
    find_mount_in(dev_path, &mount_table)
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

/// Parse mount option strings and read_only flag into libc mount flags.
#[cfg(target_os = "linux")]
fn parse_mount_flags(options: &[String], read_only: bool) -> libc::c_ulong {
    let mut flags: libc::c_ulong = 0;
    if read_only {
        flags |= libc::MS_RDONLY as libc::c_ulong;
    }
    for opt in options {
        match opt.as_str() {
            "nosuid" => flags |= libc::MS_NOSUID as libc::c_ulong,
            "nodev" => flags |= libc::MS_NODEV as libc::c_ulong,
            "noexec" => flags |= libc::MS_NOEXEC as libc::c_ulong,
            _ => {} // Unknown options are passed as mount data, not flags
        }
    }
    flags
}

/// Filesystem types to try when auto-detecting.
#[cfg(target_os = "linux")]
const AUTO_FS_TYPES: &[&str] = &[
    "ext4", "vfat", "ntfs", "iso9660", "udf", "exfat", "btrfs", "xfs",
];

/// Mount a device.
///
/// Uses the `libc::mount()` syscall. If `options.fs_type` is None, tries common
/// filesystem types in order until one succeeds.
#[cfg(target_os = "linux")]
pub fn mount(dev_path: &Path, options: &MountOptions) -> Result<MountResult> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let dev_str = dev_path.display().to_string();

    // Determine mount point
    let mount_point = match &options.mount_point {
        Some(mp) => mp.clone(),
        None => {
            // Auto-generate under /run/media/
            let dev_name = dev_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "device".to_string());
            PathBuf::from(format!("/run/media/{dev_name}"))
        }
    };

    validate_mount_point(&mount_point)?;

    // Create mount point directory if needed
    if !mount_point.exists() {
        std::fs::create_dir_all(&mount_point).map_err(|e| YantraError::MountFailed {
            device: dev_str.clone(),
            reason: format!("failed to create mount point: {e}"),
        })?;
    }

    let flags = parse_mount_flags(&options.options, options.read_only);

    let c_source =
        CString::new(dev_path.as_os_str().as_bytes()).map_err(|e| YantraError::MountFailed {
            device: dev_str.clone(),
            reason: format!("invalid device path: {e}"),
        })?;
    let c_target = CString::new(mount_point.as_os_str().as_bytes()).map_err(|e| {
        YantraError::MountFailed {
            device: dev_str.clone(),
            reason: format!("invalid mount point: {e}"),
        }
    })?;

    // Collect non-flag options to pass as mount data
    let data_options: Vec<&str> = options
        .options
        .iter()
        .filter(|o| !matches!(o.as_str(), "nosuid" | "nodev" | "noexec"))
        .map(|s| s.as_str())
        .collect();
    let data_str = data_options.join(",");
    let c_data = if data_str.is_empty() {
        None
    } else {
        Some(CString::new(data_str.as_bytes()).map_err(|e| YantraError::MountFailed {
            device: dev_str.clone(),
            reason: format!("invalid mount data: {e}"),
        })?)
    };
    let data_ptr = c_data
        .as_ref()
        .map(|c| c.as_ptr() as *const libc::c_void)
        .unwrap_or(std::ptr::null());

    let try_mount = |fs_type_str: &str| -> std::result::Result<(), i32> {
        let c_fstype = CString::new(fs_type_str).unwrap();
        let ret = unsafe {
            libc::mount(
                c_source.as_ptr(),
                c_target.as_ptr(),
                c_fstype.as_ptr(),
                flags,
                data_ptr,
            )
        };
        if ret == 0 {
            Ok(())
        } else {
            Err(unsafe { *libc::__errno_location() })
        }
    };

    if let Some(ref fs_type) = options.fs_type {
        // Explicit filesystem type
        try_mount(fs_type).map_err(|errno| map_mount_errno(errno, &dev_str))?;
        Ok(MountResult {
            device_id: DeviceId::new(dev_str),
            dev_path: dev_path.to_path_buf(),
            mount_point,
            fs_type: Filesystem::from_str_type(fs_type),
            read_only: options.read_only,
        })
    } else {
        // Auto-detect: try each filesystem type
        let mut last_errno = 0;
        for fs in AUTO_FS_TYPES {
            match try_mount(fs) {
                Ok(()) => {
                    return Ok(MountResult {
                        device_id: DeviceId::new(dev_str),
                        dev_path: dev_path.to_path_buf(),
                        mount_point,
                        fs_type: Filesystem::from_str_type(fs),
                        read_only: options.read_only,
                    });
                }
                Err(errno) => {
                    last_errno = errno;
                }
            }
        }
        Err(map_mount_errno(last_errno, &dev_str))
    }
}

/// Map errno from mount syscall to a YantraError.
#[cfg(target_os = "linux")]
fn map_mount_errno(errno: i32, device: &str) -> YantraError {
    match errno {
        libc::EPERM | libc::EACCES => YantraError::PermissionDenied {
            operation: "mount".into(),
            path: PathBuf::from(device),
        },
        libc::EBUSY => YantraError::DeviceBusy {
            path: PathBuf::from(device),
        },
        _ => {
            let err = std::io::Error::from_raw_os_error(errno);
            YantraError::MountFailed {
                device: device.to_string(),
                reason: err.to_string(),
            }
        }
    }
}

/// Map errno from umount syscall to a YantraError.
#[cfg(target_os = "linux")]
fn map_umount_errno(errno: i32, mount_point: &Path) -> YantraError {
    match errno {
        libc::EPERM | libc::EACCES => YantraError::PermissionDenied {
            operation: "unmount".into(),
            path: mount_point.to_path_buf(),
        },
        libc::EBUSY => YantraError::DeviceBusy {
            path: mount_point.to_path_buf(),
        },
        _ => {
            let err = std::io::Error::from_raw_os_error(errno);
            YantraError::UnmountFailed {
                mount_point: mount_point.to_path_buf(),
                reason: err.to_string(),
            }
        }
    }
}

/// Unmount a filesystem.
///
/// Uses `libc::umount2()` with no special flags. If the mount point is under
/// `/run/media/` and the directory is empty after unmounting, it is removed.
#[cfg(target_os = "linux")]
pub fn unmount(mount_point: &Path) -> Result<()> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let c_target = CString::new(mount_point.as_os_str().as_bytes()).map_err(|e| {
        YantraError::UnmountFailed {
            mount_point: mount_point.to_path_buf(),
            reason: format!("invalid mount point path: {e}"),
        }
    })?;

    let ret = unsafe { libc::umount2(c_target.as_ptr(), 0) };
    if ret != 0 {
        let errno = unsafe { *libc::__errno_location() };
        return Err(map_umount_errno(errno, mount_point));
    }

    // Clean up mount point directory if empty and under /run/media/
    if mount_point.starts_with("/run/media/") {
        let _ = std::fs::remove_dir(mount_point);
    }

    Ok(())
}

/// CDROMEJECT ioctl number.
#[cfg(target_os = "linux")]
const CDROMEJECT: libc::c_ulong = 0x5309;

/// Eject a device.
///
/// If the device is currently mounted, it is unmounted first.
/// - For optical drives (device name starts with "sr"): uses the CDROMEJECT ioctl.
/// - For USB/block devices: writes "1" to `/sys/block/<dev>/device/delete`.
///
/// The device is opened with `O_RDONLY | O_NONBLOCK`.
#[cfg(target_os = "linux")]
pub fn eject(dev_path: &Path) -> Result<()> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let dev_str = dev_path.display().to_string();

    // Unmount first if mounted
    if let Some(mp) = find_mount_point(dev_path) {
        unmount(&mp)?;
    }

    let dev_name = dev_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    // Determine the base device name (strip partition number for sysfs lookup)
    let base_dev_name: String = dev_name
        .trim_end_matches(|c: char| c.is_ascii_digit())
        .to_string();

    if dev_name.starts_with("sr") {
        // Optical drive — use CDROMEJECT ioctl
        let c_path = CString::new(dev_path.as_os_str().as_bytes()).map_err(|e| {
            YantraError::EjectFailed {
                device: dev_str.clone(),
                reason: format!("invalid device path: {e}"),
            }
        })?;

        let fd = unsafe { libc::open(c_path.as_ptr(), libc::O_RDONLY | libc::O_NONBLOCK) };
        if fd < 0 {
            let errno = unsafe { *libc::__errno_location() };
            return Err(YantraError::EjectFailed {
                device: dev_str,
                reason: std::io::Error::from_raw_os_error(errno).to_string(),
            });
        }

        let ret = unsafe { libc::ioctl(fd, CDROMEJECT, 0) };
        let ioctl_errno = if ret != 0 {
            Some(unsafe { *libc::__errno_location() })
        } else {
            None
        };

        unsafe {
            libc::close(fd);
        }

        if let Some(errno) = ioctl_errno {
            return Err(YantraError::EjectFailed {
                device: dev_str,
                reason: std::io::Error::from_raw_os_error(errno).to_string(),
            });
        }
    } else {
        // USB/block device — write "1" to sysfs delete
        let delete_path = format!("/sys/block/{base_dev_name}/device/delete");
        std::fs::write(&delete_path, b"1").map_err(|e| YantraError::EjectFailed {
            device: dev_str,
            reason: format!("failed to write to {delete_path}: {e}"),
        })?;
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

    // --- Tests for unescape_mount_path ---

    #[test]
    fn test_unescape_mount_path_no_escapes() {
        assert_eq!(unescape_mount_path("/mnt/usb"), "/mnt/usb");
    }

    #[test]
    fn test_unescape_mount_path_space() {
        assert_eq!(unescape_mount_path("/mnt/my\\040drive"), "/mnt/my drive");
    }

    #[test]
    fn test_unescape_mount_path_tab() {
        assert_eq!(unescape_mount_path("/mnt/my\\011drive"), "/mnt/my\tdrive");
    }

    #[test]
    fn test_unescape_mount_path_newline() {
        assert_eq!(
            unescape_mount_path("/mnt/my\\012drive"),
            "/mnt/my\ndrive"
        );
    }

    #[test]
    fn test_unescape_mount_path_backslash() {
        assert_eq!(
            unescape_mount_path("/mnt/my\\134drive"),
            "/mnt/my\\drive"
        );
    }

    #[test]
    fn test_unescape_mount_path_multiple() {
        assert_eq!(
            unescape_mount_path("/mnt/my\\040cool\\040drive"),
            "/mnt/my cool drive"
        );
    }

    #[test]
    fn test_unescape_mount_path_trailing_backslash() {
        // Backslash at end without enough octal digits — kept literal
        assert_eq!(unescape_mount_path("/mnt/test\\"), "/mnt/test\\");
    }

    #[test]
    fn test_unescape_mount_path_non_octal_after_backslash() {
        // Backslash followed by non-octal characters — kept literal
        assert_eq!(unescape_mount_path("/mnt/test\\abc"), "/mnt/test\\abc");
    }

    // --- Tests for find_mount_in ---

    #[test]
    fn test_find_mount_in_simple_match() {
        let table = "\
/dev/sda1 / ext4 rw,relatime 0 0
/dev/sdb1 /mnt/usb vfat rw,nosuid 0 0
";
        // Note: find_mount_in will try to canonicalize, which will fail for
        // non-existent /dev/sdb1 on the test host, so it falls back to the raw path.
        let result = find_mount_in(Path::new("/dev/sdb1"), table);
        assert_eq!(result, Some(PathBuf::from("/mnt/usb")));
    }

    #[test]
    fn test_find_mount_in_no_match() {
        let table = "\
/dev/sda1 / ext4 rw,relatime 0 0
/dev/sdb1 /mnt/usb vfat rw,nosuid 0 0
";
        let result = find_mount_in(Path::new("/dev/sdc1"), table);
        assert!(result.is_none());
    }

    #[test]
    fn test_find_mount_in_escaped_mountpoint() {
        let table = "/dev/sdb1 /mnt/my\\040drive vfat rw 0 0\n";
        let result = find_mount_in(Path::new("/dev/sdb1"), table);
        assert_eq!(result, Some(PathBuf::from("/mnt/my drive")));
    }

    #[test]
    fn test_find_mount_in_empty_table() {
        let result = find_mount_in(Path::new("/dev/sdb1"), "");
        assert!(result.is_none());
    }

    #[test]
    fn test_find_mount_in_comment_lines() {
        let table = "\
# this is a comment
/dev/sda1 / ext4 rw 0 0
";
        let result = find_mount_in(Path::new("/dev/sdb1"), table);
        assert!(result.is_none());
    }

    #[test]
    fn test_find_mount_in_malformed_line() {
        // Line with only one field — should be skipped gracefully
        let table = "/dev/sda1\n/dev/sdb1 /mnt/usb vfat rw 0 0\n";
        let result = find_mount_in(Path::new("/dev/sdb1"), table);
        assert_eq!(result, Some(PathBuf::from("/mnt/usb")));
    }

    #[test]
    fn test_find_mount_in_multiple_mounts_returns_first() {
        let table = "\
/dev/sdb1 /mnt/first vfat rw 0 0
/dev/sdb1 /mnt/second vfat rw 0 0
";
        let result = find_mount_in(Path::new("/dev/sdb1"), table);
        assert_eq!(result, Some(PathBuf::from("/mnt/first")));
    }

    // --- Tests for parse_mount_flags (Linux only) ---

    #[cfg(target_os = "linux")]
    #[test]
    fn test_parse_mount_flags_empty() {
        let flags = parse_mount_flags(&[], false);
        assert_eq!(flags, 0);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_parse_mount_flags_read_only() {
        let flags = parse_mount_flags(&[], true);
        assert_eq!(flags, libc::MS_RDONLY as libc::c_ulong);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_parse_mount_flags_nosuid() {
        let flags = parse_mount_flags(&["nosuid".into()], false);
        assert_eq!(flags, libc::MS_NOSUID as libc::c_ulong);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_parse_mount_flags_nodev() {
        let flags = parse_mount_flags(&["nodev".into()], false);
        assert_eq!(flags, libc::MS_NODEV as libc::c_ulong);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_parse_mount_flags_noexec() {
        let flags = parse_mount_flags(&["noexec".into()], false);
        assert_eq!(flags, libc::MS_NOEXEC as libc::c_ulong);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_parse_mount_flags_combined() {
        let flags =
            parse_mount_flags(&["nosuid".into(), "nodev".into(), "noexec".into()], true);
        let expected = libc::MS_RDONLY as libc::c_ulong
            | libc::MS_NOSUID as libc::c_ulong
            | libc::MS_NODEV as libc::c_ulong
            | libc::MS_NOEXEC as libc::c_ulong;
        assert_eq!(flags, expected);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_parse_mount_flags_unknown_option_ignored() {
        let flags = parse_mount_flags(&["uid=1000".into(), "nosuid".into()], false);
        assert_eq!(flags, libc::MS_NOSUID as libc::c_ulong);
    }

    // --- Hardware-dependent tests (require root and real devices) ---

    #[test]
    #[ignore]
    fn test_mount_real_device() {
        // Requires root and a real device at /dev/sdb1
        let opts = MountOptions {
            mount_point: Some(PathBuf::from("/tmp/yantra_test_mount")),
            read_only: true,
            options: vec!["nosuid".into(), "nodev".into()],
            fs_type: Some("vfat".into()),
        };
        let result = mount(Path::new("/dev/sdb1"), &opts);
        if let Ok(mr) = &result {
            let _ = unmount(&mr.mount_point);
        }
        // We don't assert success — just that it doesn't panic
    }

    #[test]
    #[ignore]
    fn test_unmount_real_device() {
        // Requires a mounted filesystem at /tmp/yantra_test_mount
        let result = unmount(Path::new("/tmp/yantra_test_mount"));
        // We don't assert success — just that it doesn't panic
        let _ = result;
    }

    #[test]
    #[ignore]
    fn test_eject_optical() {
        // Requires root and an optical drive at /dev/sr0
        let result = eject(Path::new("/dev/sr0"));
        let _ = result;
    }

    #[test]
    #[ignore]
    fn test_eject_usb() {
        // Requires root and a USB device at /dev/sdb
        let result = eject(Path::new("/dev/sdb"));
        let _ = result;
    }

    #[test]
    #[ignore]
    fn test_find_mount_point_real() {
        // Root filesystem should always be mounted
        let result = find_mount_point(Path::new("/dev/sda1"));
        // May or may not find it depending on actual system
        let _ = result;
    }
}
