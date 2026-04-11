//! Error types for yukti.

use std::path::PathBuf;

/// All errors that yukti can produce.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum YuktiError {
    #[error("device not found: {id}")]
    DeviceNotFound { id: String },

    #[error("device busy: {path}")]
    DeviceBusy { path: PathBuf },

    #[error("mount failed for {device}: {reason}")]
    MountFailed { device: String, reason: String },

    #[error("unmount failed for {mount_point}: {reason}")]
    UnmountFailed {
        mount_point: PathBuf,
        reason: String,
    },

    #[error("eject failed for {device}: {reason}")]
    EjectFailed { device: String, reason: String },

    #[error("tray operation failed: {reason}")]
    TrayFailed { reason: String },

    #[error("permission denied: {operation} on {path}")]
    PermissionDenied { operation: String, path: PathBuf },

    #[error("udev error: {0}")]
    Udev(String),

    #[error("device already mounted: {device} at {mount_point}")]
    AlreadyMounted {
        device: String,
        mount_point: PathBuf,
    },

    #[error("operation timed out: {operation}")]
    Timeout { operation: String },

    #[error("udev socket error: {0}")]
    UdevSocket(String),

    #[error("udev parse error: {0}")]
    UdevParse(String),

    #[error("no media in device {device}")]
    NoMedia { device: String },

    #[error("unsupported filesystem: {fs_type}")]
    UnsupportedFilesystem { fs_type: String },

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("parse error: {0}")]
    Parse(String),
}

pub type Result<T> = std::result::Result<T, YuktiError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_not_found() {
        let e = YuktiError::DeviceNotFound {
            id: "usb-001".into(),
        };
        assert!(e.to_string().contains("usb-001"));
    }

    #[test]
    fn test_device_busy() {
        let e = YuktiError::DeviceBusy {
            path: PathBuf::from("/dev/sdb1"),
        };
        assert!(e.to_string().contains("/dev/sdb1"));
    }

    #[test]
    fn test_mount_failed() {
        let e = YuktiError::MountFailed {
            device: "/dev/sdb1".into(),
            reason: "bad superblock".into(),
        };
        assert!(e.to_string().contains("/dev/sdb1"));
        assert!(e.to_string().contains("bad superblock"));
    }

    #[test]
    fn test_unmount_failed() {
        let e = YuktiError::UnmountFailed {
            mount_point: PathBuf::from("/mnt/usb"),
            reason: "device busy".into(),
        };
        assert!(e.to_string().contains("/mnt/usb"));
        assert!(e.to_string().contains("device busy"));
    }

    #[test]
    fn test_eject_failed() {
        let e = YuktiError::EjectFailed {
            device: "/dev/sr0".into(),
            reason: "tray locked".into(),
        };
        assert!(e.to_string().contains("/dev/sr0"));
        assert!(e.to_string().contains("tray locked"));
    }

    #[test]
    fn test_tray_failed() {
        let e = YuktiError::TrayFailed {
            reason: "mechanical error".into(),
        };
        assert!(e.to_string().contains("mechanical error"));
    }

    #[test]
    fn test_permission_denied() {
        let e = YuktiError::PermissionDenied {
            operation: "mount".into(),
            path: PathBuf::from("/dev/sda"),
        };
        assert!(e.to_string().contains("permission denied"));
        assert!(e.to_string().contains("mount"));
        assert!(e.to_string().contains("/dev/sda"));
    }

    #[test]
    fn test_udev_error() {
        let e = YuktiError::Udev("netlink failed".into());
        assert!(e.to_string().contains("netlink failed"));
    }

    #[test]
    fn test_no_media() {
        let e = YuktiError::NoMedia {
            device: "/dev/sr0".into(),
        };
        assert!(e.to_string().contains("no media"));
    }

    #[test]
    fn test_unsupported_filesystem() {
        let e = YuktiError::UnsupportedFilesystem {
            fs_type: "hammerfs".into(),
        };
        assert!(e.to_string().contains("hammerfs"));
    }

    #[test]
    fn test_io_error_from() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let e: YuktiError = io_err.into();
        assert!(e.to_string().contains("file not found"));
    }

    #[test]
    fn test_parse_error() {
        let e = YuktiError::Parse("invalid integer".into());
        assert!(e.to_string().contains("invalid integer"));
    }

    #[test]
    fn test_already_mounted() {
        let e = YuktiError::AlreadyMounted {
            device: "/dev/sdb1".into(),
            mount_point: PathBuf::from("/mnt/usb"),
        };
        let msg = e.to_string();
        assert!(msg.contains("device already mounted"));
        assert!(msg.contains("/dev/sdb1"));
        assert!(msg.contains("/mnt/usb"));
    }

    #[test]
    fn test_timeout() {
        let e = YuktiError::Timeout {
            operation: "mount /dev/sdb1".into(),
        };
        let msg = e.to_string();
        assert!(msg.contains("operation timed out"));
        assert!(msg.contains("mount /dev/sdb1"));
    }

    #[test]
    fn test_udev_socket_error() {
        let e = YuktiError::UdevSocket("bind() failed: Address in use".into());
        let msg = e.to_string();
        assert!(msg.contains("udev socket error"));
        assert!(msg.contains("bind() failed"));
    }

    #[test]
    fn test_udev_parse_error() {
        let e = YuktiError::UdevParse("invalid uevent format".into());
        let msg = e.to_string();
        assert!(msg.contains("udev parse error"));
        assert!(msg.contains("invalid uevent format"));
    }

    #[test]
    fn test_result_type_alias() {
        let ok: Result<i32> = Ok(42);
        assert!(ok.is_ok());

        let err: Result<i32> = Err(YuktiError::Parse("bad".into()));
        assert!(err.is_err());
    }
}
