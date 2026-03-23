//! Linux device manager — ties together udev, storage, and optical modules.
//!
//! Provides a concrete implementation of the [`Device`] trait that reads
//! from sysfs, monitors udev events via netlink, and delegates mount/eject
//! operations to the appropriate subsystem functions.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

use crate::device::{Device, DeviceId, DeviceInfo};
use crate::error::{Result, YantraError};
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
    devices: RwLock<HashMap<DeviceId, DeviceInfo>>,
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
            return Err(YantraError::Udev("monitor already running".into()));
        }
        let mon = UdevMonitor::new()?;
        let rx = mon.subscribe();
        *guard = Some(mon);
        Ok(rx)
    }

    /// Stop the background udev monitor.
    #[cfg(feature = "udev")]
    pub fn stop_monitor(&self) {
        let guard = self.monitor.lock().unwrap();
        if let Some(ref mon) = *guard {
            mon.stop();
        }
    }

    /// Mount a device by ID.
    #[cfg(feature = "storage")]
    pub fn mount(&self, id: &DeviceId, options: &MountOptions) -> Result<MountResult> {
        let devices = self.devices.read().unwrap();
        let info = devices.get(id).ok_or_else(|| YantraError::DeviceNotFound {
            id: id.to_string(),
        })?;
        let result = crate::storage::mount(&info.dev_path, options)?;

        drop(devices);
        // Update cached state
        let mut devices = self.devices.write().unwrap();
        if let Some(info) = devices.get_mut(id) {
            info.state = crate::device::DeviceState::Mounted;
            info.mount_point = Some(result.mount_point.clone());
        }
        Ok(result)
    }

    /// Unmount a device by ID.
    #[cfg(feature = "storage")]
    pub fn unmount(&self, id: &DeviceId) -> Result<()> {
        let devices = self.devices.read().unwrap();
        let info = devices.get(id).ok_or_else(|| YantraError::DeviceNotFound {
            id: id.to_string(),
        })?;
        let mount_point = info.mount_point.clone().ok_or_else(|| {
            YantraError::UnmountFailed {
                mount_point: info.dev_path.clone(),
                reason: "device is not mounted".into(),
            }
        })?;
        drop(devices);

        crate::storage::unmount(&mount_point)?;

        let mut devices = self.devices.write().unwrap();
        if let Some(info) = devices.get_mut(id) {
            info.state = crate::device::DeviceState::Ready;
            info.mount_point = None;
        }
        Ok(())
    }

    /// Eject a device by ID (safe-remove for USB, tray eject for optical).
    #[cfg(feature = "storage")]
    pub fn eject(&self, id: &DeviceId) -> Result<()> {
        let devices = self.devices.read().unwrap();
        let info = devices.get(id).ok_or_else(|| YantraError::DeviceNotFound {
            id: id.to_string(),
        })?;
        let dev_path = info.dev_path.clone();
        drop(devices);

        crate::storage::eject(&dev_path)?;

        let mut devices = self.devices.write().unwrap();
        if let Some(info) = devices.get_mut(id) {
            info.state = crate::device::DeviceState::Ejecting;
        }
        Ok(())
    }

    /// Get the sysfs root path.
    pub fn sysfs_root(&self) -> &Path {
        &self.sysfs_root
    }

    /// Dispatch a device event to all registered listeners.
    pub fn dispatch_event(&self, event: &DeviceEvent) {
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
        #[cfg(feature = "udev")]
        {
            let devices = enumerate_devices(&self.sysfs_root)?;
            let mut cache = self.devices.write().unwrap();
            cache.clear();
            for device in &devices {
                cache.insert(device.id.clone(), device.clone());
            }
            Ok(devices)
        }
        #[cfg(not(feature = "udev"))]
        {
            Ok(Vec::new())
        }
    }

    fn get(&self, id: &DeviceId) -> Result<Option<DeviceInfo>> {
        let devices = self.devices.read().unwrap();
        Ok(devices.get(id).cloned())
    }

    fn refresh(&self, id: &DeviceId) -> Result<DeviceInfo> {
        // Re-enumerate and find the device
        let all = self.enumerate()?;
        all.into_iter()
            .find(|d| d.id == *id)
            .ok_or_else(|| YantraError::DeviceNotFound {
                id: id.to_string(),
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
        let dir = std::env::temp_dir().join("yantra_test_empty_sysfs");
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
}
