//! Error types for yantra.

use std::path::PathBuf;

/// All errors that yantra can produce.
#[derive(Debug, thiserror::Error)]
pub enum YantraError {
    #[error("device not found: {id}")]
    DeviceNotFound { id: String },

    #[error("device busy: {path}")]
    DeviceBusy { path: PathBuf },

    #[error("mount failed for {device}: {reason}")]
    MountFailed { device: String, reason: String },

    #[error("unmount failed for {mount_point}: {reason}")]
    UnmountFailed { mount_point: PathBuf, reason: String },

    #[error("eject failed for {device}: {reason}")]
    EjectFailed { device: String, reason: String },

    #[error("tray operation failed: {reason}")]
    TrayFailed { reason: String },

    #[error("permission denied: {operation} on {path}")]
    PermissionDenied { operation: String, path: PathBuf },

    #[error("udev error: {0}")]
    Udev(String),

    #[error("no media in device {device}")]
    NoMedia { device: String },

    #[error("unsupported filesystem: {fs_type}")]
    UnsupportedFilesystem { fs_type: String },

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("parse error: {0}")]
    Parse(String),
}

pub type Result<T> = std::result::Result<T, YantraError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_not_found() {
        let e = YantraError::DeviceNotFound {
            id: "usb-001".into(),
        };
        assert!(e.to_string().contains("usb-001"));
    }

    #[test]
    fn test_mount_failed() {
        let e = YantraError::MountFailed {
            device: "/dev/sdb1".into(),
            reason: "bad superblock".into(),
        };
        assert!(e.to_string().contains("/dev/sdb1"));
        assert!(e.to_string().contains("bad superblock"));
    }

    #[test]
    fn test_no_media() {
        let e = YantraError::NoMedia {
            device: "/dev/sr0".into(),
        };
        assert!(e.to_string().contains("no media"));
    }
}
