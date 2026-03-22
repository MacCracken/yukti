//! Device events — attach, detach, media change, mount/unmount.

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::device::{DeviceClass, DeviceId, DeviceInfo};

/// What happened to a device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceEventKind {
    /// New device detected (USB plugged in, disc inserted).
    Attached,
    /// Device removed (USB unplugged, disc ejected).
    Detached,
    /// Media changed (new disc in optical drive).
    MediaChanged,
    /// Device mounted at a path.
    Mounted { mount_point: PathBuf },
    /// Device unmounted.
    Unmounted,
    /// Device error (IO error, filesystem corruption).
    Error { message: String },
}

impl std::fmt::Display for DeviceEventKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Attached => f.write_str("attached"),
            Self::Detached => f.write_str("detached"),
            Self::MediaChanged => f.write_str("media-changed"),
            Self::Mounted { mount_point } => write!(f, "mounted:{}", mount_point.display()),
            Self::Unmounted => f.write_str("unmounted"),
            Self::Error { message } => write!(f, "error:{message}"),
        }
    }
}

/// A device event with full context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceEvent {
    /// Which device.
    pub device_id: DeviceId,
    /// Device class (for quick filtering without looking up full info).
    pub device_class: DeviceClass,
    /// What happened.
    pub kind: DeviceEventKind,
    /// Device path (e.g. /dev/sdb1).
    pub dev_path: PathBuf,
    /// When it happened.
    pub timestamp: DateTime<Utc>,
    /// Snapshot of device info at event time (None for detach).
    pub device_info: Option<DeviceInfo>,
}

impl DeviceEvent {
    /// Create a new event.
    pub fn new(
        device_id: DeviceId,
        device_class: DeviceClass,
        kind: DeviceEventKind,
        dev_path: PathBuf,
    ) -> Self {
        Self {
            device_id,
            device_class,
            kind,
            dev_path,
            timestamp: Utc::now(),
            device_info: None,
        }
    }

    /// Attach device info snapshot to the event.
    pub fn with_info(mut self, info: DeviceInfo) -> Self {
        self.device_info = Some(info);
        self
    }

    /// Is this an attach event?
    #[inline]
    pub fn is_attach(&self) -> bool {
        matches!(self.kind, DeviceEventKind::Attached)
    }

    /// Is this a detach event?
    #[inline]
    pub fn is_detach(&self) -> bool {
        matches!(self.kind, DeviceEventKind::Detached)
    }

    /// Is this event for a removable device?
    #[inline]
    pub fn is_removable(&self) -> bool {
        matches!(
            self.device_class,
            DeviceClass::UsbStorage | DeviceClass::Optical | DeviceClass::SdCard
        )
    }
}

/// Trait for receiving device events.
pub trait EventListener: Send + Sync {
    /// Called when a device event occurs.
    fn on_event(&self, event: &DeviceEvent);

    /// Filter: which device classes this listener cares about.
    /// Return None to receive all events.
    fn filter(&self) -> Option<Vec<DeviceClass>> {
        None
    }
}

/// Simple event collector for testing.
#[derive(Default)]
pub struct EventCollector {
    events: std::sync::Mutex<Vec<DeviceEvent>>,
}

impl EventCollector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn events(&self) -> Vec<DeviceEvent> {
        self.events.lock().unwrap().clone()
    }

    pub fn count(&self) -> usize {
        self.events.lock().unwrap().len()
    }

    pub fn clear(&self) {
        self.events.lock().unwrap().clear();
    }
}

impl EventListener for EventCollector {
    fn on_event(&self, event: &DeviceEvent) {
        self.events.lock().unwrap().push(event.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_event(kind: DeviceEventKind) -> DeviceEvent {
        DeviceEvent::new(
            DeviceId::new("block:sdb1"),
            DeviceClass::UsbStorage,
            kind,
            PathBuf::from("/dev/sdb1"),
        )
    }

    #[test]
    fn test_event_attach() {
        let e = make_event(DeviceEventKind::Attached);
        assert!(e.is_attach());
        assert!(!e.is_detach());
        assert!(e.is_removable());
    }

    #[test]
    fn test_event_detach() {
        let e = make_event(DeviceEventKind::Detached);
        assert!(e.is_detach());
        assert!(!e.is_attach());
    }

    #[test]
    fn test_event_mounted() {
        let e = make_event(DeviceEventKind::Mounted {
            mount_point: PathBuf::from("/mnt/usb"),
        });
        assert!(!e.is_attach());
        assert!(e.kind.to_string().contains("/mnt/usb"));
    }

    #[test]
    fn test_event_error_display() {
        let kind = DeviceEventKind::Error {
            message: "IO fault".into(),
        };
        assert_eq!(kind.to_string(), "error:IO fault");
    }

    #[test]
    fn test_event_display_all() {
        assert_eq!(DeviceEventKind::Attached.to_string(), "attached");
        assert_eq!(DeviceEventKind::Detached.to_string(), "detached");
        assert_eq!(DeviceEventKind::MediaChanged.to_string(), "media-changed");
        assert_eq!(DeviceEventKind::Unmounted.to_string(), "unmounted");
    }

    #[test]
    fn test_event_collector() {
        let collector = EventCollector::new();
        assert_eq!(collector.count(), 0);

        collector.on_event(&make_event(DeviceEventKind::Attached));
        collector.on_event(&make_event(DeviceEventKind::Detached));
        assert_eq!(collector.count(), 2);

        let events = collector.events();
        assert!(events[0].is_attach());
        assert!(events[1].is_detach());

        collector.clear();
        assert_eq!(collector.count(), 0);
    }

    #[test]
    fn test_event_with_info() {
        let info = DeviceInfo::new(
            DeviceId::new("block:sdb1"),
            PathBuf::from("/dev/sdb1"),
            DeviceClass::UsbStorage,
        );
        let e = make_event(DeviceEventKind::Attached).with_info(info);
        assert!(e.device_info.is_some());
    }

    #[test]
    fn test_non_removable_event() {
        let e = DeviceEvent::new(
            DeviceId::new("block:nvme0n1"),
            DeviceClass::BlockInternal,
            DeviceEventKind::Attached,
            PathBuf::from("/dev/nvme0n1"),
        );
        assert!(!e.is_removable());
    }

    #[test]
    fn test_event_listener_default_filter() {
        let collector = EventCollector::new();
        assert!(collector.filter().is_none());
    }

    #[test]
    fn test_event_collector_concurrent() {
        use std::sync::Arc;
        use std::thread;

        let collector = Arc::new(EventCollector::new());
        let mut handles = vec![];

        for _ in 0..4 {
            let c = Arc::clone(&collector);
            handles.push(thread::spawn(move || {
                for _ in 0..25 {
                    c.on_event(&DeviceEvent::new(
                        DeviceId::new("block:sdb1"),
                        DeviceClass::UsbStorage,
                        DeviceEventKind::Attached,
                        PathBuf::from("/dev/sdb1"),
                    ));
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(collector.count(), 100);
    }

    #[test]
    fn test_event_serde_roundtrip() {
        let event = make_event(DeviceEventKind::Attached);
        let json = serde_json::to_string(&event).unwrap();
        let roundtrip: DeviceEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.device_id, event.device_id);
        assert_eq!(roundtrip.kind, event.kind);
    }
}
