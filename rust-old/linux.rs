//! Linux device manager — ties together udev, storage, and optical modules.
//!
//! Provides a concrete implementation of the [`Device`] trait that reads
//! from sysfs, monitors udev events via netlink, and delegates mount/eject
//! operations to the appropriate subsystem functions.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

use tracing::{debug, error, info, warn};

use crate::device::{Device, DeviceId, DeviceInfo};
use crate::error::{Result, YuktiError};
use crate::event::{DeviceEvent, EventListener};

#[cfg(feature = "storage")]
use crate::storage::{MountOptions, MountResult};

#[cfg(feature = "udev")]
use crate::udev::{UdevMonitor, enumerate_devices};

/// Manages hardware devices on a Linux system.
///
/// Implements the [`Device`] trait for enumeration and lookup, and provides
/// higher-level operations like mount, eject, and hotplug monitoring.
pub struct LinuxDeviceManager {
    sysfs_root: PathBuf,
    devices: RwLock<HashMap<DeviceId, Arc<DeviceInfo>>>,
    listeners: Arc<Mutex<Vec<Arc<dyn EventListener>>>>,
    #[cfg(feature = "udev")]
    monitor: Mutex<Option<UdevMonitor>>,
}

impl LinuxDeviceManager {
    /// Create a new manager using the default sysfs root (`/sys`).
    pub fn new() -> Self {
        Self::with_sysfs_root(PathBuf::from("/sys"))
    }

    /// Create a new manager with a custom sysfs root (useful for testing).
    pub fn with_sysfs_root(root: PathBuf) -> Self {
        Self {
            sysfs_root: root,
            devices: RwLock::new(HashMap::new()),
            listeners: Arc::new(Mutex::new(Vec::new())),
            #[cfg(feature = "udev")]
            monitor: Mutex::new(None),
        }
    }

    /// Register an event listener for device hotplug events.
    pub fn add_listener(&self, listener: Arc<dyn EventListener>) {
        debug!("registering event listener");
        self.listeners.lock().unwrap().push(listener);
    }

    /// Start the udev hotplug monitor in a background thread.
    ///
    /// Returns a channel receiver that yields [`DeviceEvent`]s.
    /// The monitor runs until [`stop_monitor()`](Self::stop_monitor) is called
    /// or the manager is dropped.
    #[cfg(feature = "udev")]
    pub fn start_monitor(&self) -> Result<std::sync::mpsc::Receiver<DeviceEvent>> {
        let mut guard = self.monitor.lock().unwrap();
        if guard.is_some() {
            warn!("start_monitor called but monitor already running");
            return Err(YuktiError::Udev("monitor already running".into()));
        }
        info!("starting udev hotplug monitor");
        let mon = UdevMonitor::new()?;
        let rx = mon.subscribe();
        *guard = Some(mon);
        Ok(rx)
    }

    /// Stop the background udev monitor.
    #[cfg(feature = "udev")]
    pub fn stop_monitor(&self) {
        info!("stopping udev hotplug monitor");
        let guard = self.monitor.lock().unwrap();
        if let Some(ref mon) = *guard {
            mon.stop();
        }
    }

    /// Mount a device by ID.
    #[cfg(feature = "storage")]
    pub fn mount(&self, id: &DeviceId, options: &MountOptions) -> Result<MountResult> {
        let info = {
            let devices = self.devices.read().unwrap();
            Arc::clone(
                devices
                    .get(id)
                    .ok_or_else(|| YuktiError::DeviceNotFound { id: id.to_string() })?,
            )
        };
        let result = crate::storage::mount(&info.dev_path, options)?;

        // Update cached state — replace the Arc entry to avoid Arc::make_mut clone
        let mut devices = self.devices.write().unwrap();
        if let Some(cached) = devices.get_mut(id) {
            let mut updated = (**cached).clone();
            updated.state = crate::device::DeviceState::Mounted;
            updated.mount_point = Some(result.mount_point.clone());
            *cached = Arc::new(updated);
        }
        Ok(result)
    }

    /// Unmount a device by ID.
    #[cfg(feature = "storage")]
    pub fn unmount(&self, id: &DeviceId) -> Result<()> {
        let info = {
            let devices = self.devices.read().unwrap();
            Arc::clone(
                devices
                    .get(id)
                    .ok_or_else(|| YuktiError::DeviceNotFound { id: id.to_string() })?,
            )
        };
        let mount_point = info
            .mount_point
            .clone()
            .ok_or_else(|| YuktiError::UnmountFailed {
                mount_point: info.dev_path.clone(),
                reason: "device is not mounted".into(),
            })?;

        crate::storage::unmount(&mount_point)?;

        let mut devices = self.devices.write().unwrap();
        if let Some(cached) = devices.get_mut(id) {
            let mut updated = (**cached).clone();
            updated.state = crate::device::DeviceState::Ready;
            updated.mount_point = None;
            *cached = Arc::new(updated);
        }
        Ok(())
    }

    /// Eject a device by ID (safe-remove for USB, tray eject for optical).
    #[cfg(feature = "storage")]
    pub fn eject(&self, id: &DeviceId) -> Result<()> {
        let info = {
            let devices = self.devices.read().unwrap();
            Arc::clone(
                devices
                    .get(id)
                    .ok_or_else(|| YuktiError::DeviceNotFound { id: id.to_string() })?,
            )
        };

        crate::storage::eject(&info.dev_path)?;

        let mut devices = self.devices.write().unwrap();
        if let Some(cached) = devices.get_mut(id) {
            let mut updated = (**cached).clone();
            updated.state = crate::device::DeviceState::Ejecting;
            *cached = Arc::new(updated);
        }
        Ok(())
    }

    /// Get the sysfs root path.
    pub fn sysfs_root(&self) -> &Path {
        &self.sysfs_root
    }

    /// Dispatch a device event to all registered listeners.
    pub fn dispatch_event(&self, event: &DeviceEvent) {
        debug!(device_id = %event.device_id, kind = %event.kind, class = %event.device_class, "dispatching event");
        let listeners = self.listeners.lock().unwrap();
        for listener in listeners.iter() {
            if let Some(filter) = listener.filter()
                && !filter.contains(&event.device_class)
            {
                continue;
            }
            listener.on_event(event);
        }
    }
}

impl Default for LinuxDeviceManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Device for LinuxDeviceManager {
    fn enumerate(&self) -> Result<Vec<DeviceInfo>> {
        info!(sysfs_root = %self.sysfs_root.display(), "enumerating devices");
        #[cfg(feature = "udev")]
        {
            let devices = enumerate_devices(&self.sysfs_root)?;
            let mut cache = self.devices.write().unwrap();
            cache.clear();
            for device in &devices {
                cache.insert(device.id.clone(), Arc::new(device.clone()));
            }
            info!(count = devices.len(), "device cache updated");
            Ok(devices)
        }
        #[cfg(not(feature = "udev"))]
        {
            warn!("udev feature disabled, enumeration unavailable");
            Ok(Vec::new())
        }
    }

    fn get(&self, id: &DeviceId) -> Result<Option<DeviceInfo>> {
        let devices = self.devices.read().unwrap();
        let found = devices.get(id).map(|arc| arc.as_ref().clone());
        debug!(id = %id, found = found.is_some(), "device lookup");
        Ok(found)
    }

    fn refresh(&self, id: &DeviceId) -> Result<DeviceInfo> {
        debug!(id = %id, "refreshing device");
        let all = self.enumerate()?;
        all.into_iter().find(|d| d.id == *id).ok_or_else(|| {
            error!(id = %id, "device not found after refresh");
            YuktiError::DeviceNotFound { id: id.to_string() }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::EventCollector;
    use std::sync::Arc;

    #[test]
    fn test_new_default() {
        let mgr = LinuxDeviceManager::new();
        assert_eq!(mgr.sysfs_root(), Path::new("/sys"));
    }

    #[test]
    fn test_custom_sysfs_root() {
        let mgr = LinuxDeviceManager::with_sysfs_root(PathBuf::from("/tmp/fake_sys"));
        assert_eq!(mgr.sysfs_root(), Path::new("/tmp/fake_sys"));
    }

    #[test]
    fn test_get_nonexistent() {
        let mgr = LinuxDeviceManager::new();
        let result = mgr.get(&DeviceId::new("nonexistent"));
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_refresh_nonexistent() {
        let mgr = LinuxDeviceManager::with_sysfs_root(PathBuf::from("/tmp/empty_sysfs"));
        let result = mgr.refresh(&DeviceId::new("nonexistent"));
        assert!(result.is_err());
    }

    #[test]
    fn test_add_listener() {
        let mgr = LinuxDeviceManager::new();
        let collector = Arc::new(EventCollector::new());
        mgr.add_listener(collector.clone());

        let event = DeviceEvent::new(
            DeviceId::new("test"),
            crate::device::DeviceClass::UsbStorage,
            crate::event::DeviceEventKind::Attached,
            PathBuf::from("/dev/sdb1"),
        );
        mgr.dispatch_event(&event);
        assert_eq!(collector.count(), 1);
    }

    #[test]
    fn test_listener_filter() {
        let mgr = LinuxDeviceManager::new();

        // Custom listener that only wants optical events
        struct OpticalOnly;
        impl EventListener for OpticalOnly {
            fn on_event(&self, _event: &DeviceEvent) {}
            fn filter(&self) -> Option<Vec<crate::device::DeviceClass>> {
                Some(vec![crate::device::DeviceClass::Optical])
            }
        }

        let collector = Arc::new(EventCollector::new());
        mgr.add_listener(collector.clone());
        mgr.add_listener(Arc::new(OpticalOnly));

        // USB event — collector gets it, OpticalOnly doesn't
        let usb_event = DeviceEvent::new(
            DeviceId::new("test"),
            crate::device::DeviceClass::UsbStorage,
            crate::event::DeviceEventKind::Attached,
            PathBuf::from("/dev/sdb1"),
        );
        mgr.dispatch_event(&usb_event);
        assert_eq!(collector.count(), 1);
    }

    #[test]
    fn test_enumerate_empty_sysfs() {
        let dir = std::env::temp_dir().join("yukti_test_empty_sysfs");
        let _ = std::fs::create_dir_all(dir.join("block"));
        let mgr = LinuxDeviceManager::with_sysfs_root(dir.clone());
        let devices = mgr.enumerate().unwrap();
        assert!(devices.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_default_trait() {
        let mgr = LinuxDeviceManager::default();
        assert_eq!(mgr.sysfs_root(), Path::new("/sys"));
    }

    #[cfg(feature = "udev")]
    #[test]
    fn test_double_start_monitor_fails() {
        // Can't actually start a monitor without root, but we can test
        // that starting twice errors if the first succeeds.
        // This test is best-effort — if new() fails due to permissions, skip.
        let mgr = LinuxDeviceManager::new();
        if let Ok(_rx) = mgr.start_monitor() {
            let second = mgr.start_monitor();
            assert!(second.is_err());
            mgr.stop_monitor();
        }
    }

    // -----------------------------------------------------------------------
    // Integration tests using MockSysfs
    // -----------------------------------------------------------------------

    #[cfg(feature = "udev")]
    mod mock_integration {
        use super::*;

        #[derive(Default)]
        struct MockDeviceAttrs {
            vendor: Option<String>,
            model: Option<String>,
            #[allow(dead_code)]
            serial: Option<String>,
            size_sectors: Option<u64>,
            removable: bool,
            #[allow(dead_code)]
            readonly: bool,
        }

        struct MockSysfs {
            root: tempfile::TempDir,
        }

        impl MockSysfs {
            fn new() -> Self {
                let root = tempfile::TempDir::new().expect("failed to create temp dir");
                std::fs::create_dir_all(root.path().join("block")).unwrap();
                Self { root }
            }

            fn root(&self) -> &Path {
                self.root.path()
            }

            fn add_device(&self, name: &str, attrs: MockDeviceAttrs) {
                let dev_dir = self.root.path().join("block").join(name);
                std::fs::create_dir_all(dev_dir.join("device")).unwrap();
                Self::write_attrs(&dev_dir, &attrs);
            }

            fn write_attrs(dir: &Path, attrs: &MockDeviceAttrs) {
                if let Some(ref vendor) = attrs.vendor {
                    std::fs::write(dir.join("device/vendor"), format!("  {}  \n", vendor)).unwrap();
                }
                if let Some(ref model) = attrs.model {
                    std::fs::write(dir.join("device/model"), format!("{}\n", model)).unwrap();
                }
                if let Some(ref serial) = attrs.serial {
                    std::fs::write(dir.join("device/serial"), format!("{}\n", serial)).unwrap();
                }
                if let Some(sectors) = attrs.size_sectors {
                    std::fs::write(dir.join("size"), format!("{}\n", sectors)).unwrap();
                }
                std::fs::write(
                    dir.join("removable"),
                    if attrs.removable { "1\n" } else { "0\n" },
                )
                .unwrap();
                std::fs::write(dir.join("ro"), if attrs.readonly { "1\n" } else { "0\n" }).unwrap();
            }
        }

        #[test]
        fn test_manager_enumerate_populates_cache() {
            let mock = MockSysfs::new();
            mock.add_device(
                "sda",
                MockDeviceAttrs {
                    vendor: Some("ATA".into()),
                    model: Some("TestDisk".into()),
                    size_sectors: Some(2000),
                    ..Default::default()
                },
            );
            mock.add_device(
                "sdb",
                MockDeviceAttrs {
                    size_sectors: Some(1000),
                    removable: true,
                    ..Default::default()
                },
            );

            let mgr = LinuxDeviceManager::with_sysfs_root(mock.root().to_path_buf());
            let devices = mgr.enumerate().unwrap();
            assert_eq!(devices.len(), 2);

            // Cache should now be populated
            let cache = mgr.devices.read().unwrap();
            assert_eq!(cache.len(), 2);
        }

        #[test]
        fn test_manager_get_by_id_after_enumerate() {
            let mock = MockSysfs::new();
            mock.add_device(
                "sda",
                MockDeviceAttrs {
                    vendor: Some("TestVendor".into()),
                    size_sectors: Some(5000),
                    ..Default::default()
                },
            );

            let mgr = LinuxDeviceManager::with_sysfs_root(mock.root().to_path_buf());
            let devices = mgr.enumerate().unwrap();
            assert_eq!(devices.len(), 1);

            // Retrieve by the ID assigned during enumeration
            let id = &devices[0].id;
            let fetched = mgr.get(id).unwrap();
            assert!(fetched.is_some());
            let fetched = fetched.unwrap();
            assert_eq!(fetched.vendor.as_deref(), Some("TestVendor"));
            assert_eq!(fetched.size_bytes, 5000 * 512);
        }

        #[test]
        fn test_manager_refresh_after_enumerate() {
            let mock = MockSysfs::new();
            mock.add_device(
                "sda",
                MockDeviceAttrs {
                    size_sectors: Some(1000),
                    ..Default::default()
                },
            );

            let mgr = LinuxDeviceManager::with_sysfs_root(mock.root().to_path_buf());
            let devices = mgr.enumerate().unwrap();
            let id = devices[0].id.clone();

            // Refresh should re-enumerate and find the device
            let refreshed = mgr.refresh(&id).unwrap();
            assert_eq!(refreshed.id, id);
            assert_eq!(refreshed.size_bytes, 1000 * 512);
        }

        #[test]
        fn test_manager_enumerate_empty_mock() {
            let mock = MockSysfs::new();
            let mgr = LinuxDeviceManager::with_sysfs_root(mock.root().to_path_buf());
            let devices = mgr.enumerate().unwrap();
            assert!(devices.is_empty());

            let cache = mgr.devices.read().unwrap();
            assert!(cache.is_empty());
        }
    }
}
