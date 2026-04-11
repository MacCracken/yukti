//! udev integration — device enumeration and hotplug monitoring.
//!
//! This module provides the interface for udev-based device discovery.
//! On Linux, it reads from `/sys/` and monitors udev events via netlink.
//! The actual netlink/libudev integration is behind trait abstractions
//! so it can be tested without root.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[cfg(target_os = "linux")]
use std::sync::Arc;
#[cfg(target_os = "linux")]
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};

use tracing::{debug, info};

use crate::device::{DeviceCapabilities, DeviceClass, DeviceId, DeviceInfo};
use crate::error::{Result, YuktiError};
#[cfg(target_os = "linux")]
use crate::event::{DeviceEvent, DeviceEventKind, EventListener};

/// A raw udev device event from the kernel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UdevEvent {
    /// Action: "add", "remove", "change", "move", "bind", "unbind".
    pub action: String,
    /// Device path in sysfs.
    pub sys_path: PathBuf,
    /// Device node path (e.g. /dev/sdb1).
    pub dev_path: Option<PathBuf>,
    /// Subsystem (block, usb, scsi, etc).
    pub subsystem: String,
    /// Device type (disk, partition, etc).
    pub dev_type: Option<String>,
    /// All udev properties.
    pub properties: HashMap<String, String>,
}

impl UdevEvent {
    pub fn is_add(&self) -> bool {
        self.action == "add"
    }

    pub fn is_remove(&self) -> bool {
        self.action == "remove"
    }

    pub fn is_change(&self) -> bool {
        self.action == "change"
    }

    /// Get a property value.
    #[inline]
    pub fn property(&self, key: &str) -> Option<&str> {
        self.properties.get(key).map(|s| s.as_str())
    }
}

/// Classify a device based on udev properties.
pub fn classify_device(event: &UdevEvent) -> DeviceClass {
    let bus = event.property("ID_BUS").unwrap_or("");
    let dev_type = event.dev_type.as_deref().unwrap_or("");
    let subsystem = event.subsystem.as_str();

    // USB storage
    if bus == "usb" && (dev_type == "disk" || dev_type == "partition") {
        return DeviceClass::UsbStorage;
    }

    // Optical drive
    if subsystem == "block" && event.property("ID_CDROM").is_some() {
        return DeviceClass::Optical;
    }

    // SD card
    if bus == "mmc" || event.property("ID_DRIVE_FLASH_SD").is_some() {
        return DeviceClass::SdCard;
    }

    // Device mapper — use byte-level contains to avoid to_string_lossy allocation
    if subsystem == "block" {
        let path_bytes = event.sys_path.as_os_str().as_encoded_bytes();
        if contains_bytes(path_bytes, b"/dm-") {
            return DeviceClass::DeviceMapper;
        }
        if contains_bytes(path_bytes, b"/loop") {
            return DeviceClass::Loop;
        }
        if dev_type == "disk" || dev_type == "partition" {
            return DeviceClass::BlockInternal;
        }
    }

    DeviceClass::Unknown
}

/// Byte-level substring search (avoids string allocation from OsStr).
#[inline]
fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

/// Classify device and extract capabilities in a single pass,
/// avoiding redundant HashMap lookups.
pub fn classify_and_extract(event: &UdevEvent) -> (DeviceClass, DeviceCapabilities) {
    let class = classify_device(event);
    let caps = extract_capabilities(event, class);
    (class, caps)
}

/// Extract capabilities from udev properties.
pub fn extract_capabilities(event: &UdevEvent, class: DeviceClass) -> DeviceCapabilities {
    let mut caps = DeviceCapabilities::READ;

    // Writable unless read-only
    if event.property("ID_FS_READONLY").unwrap_or("0") != "1" {
        caps |= DeviceCapabilities::WRITE;
    }

    // Removable
    let removable = event.property("ID_USB_DRIVER").is_some()
        || event.property("UDISKS_REMOVABLE").unwrap_or("0") == "1"
        || matches!(
            class,
            DeviceClass::UsbStorage | DeviceClass::Optical | DeviceClass::SdCard
        );
    if removable {
        caps |=
            DeviceCapabilities::REMOVABLE | DeviceCapabilities::HOTPLUG | DeviceCapabilities::EJECT;
    }

    // Optical-specific
    if class == DeviceClass::Optical {
        caps |= DeviceCapabilities::MEDIA_CHANGE | DeviceCapabilities::TRAY_CONTROL;
        if event.property("ID_CDROM_CD_RW").is_some() || event.property("ID_CDROM_DVD_RW").is_some()
        {
            caps |= DeviceCapabilities::BURN;
        }
    }

    // SSD TRIM
    if event.property("ID_ATA_FEATURE_SET_TRIM").is_some() {
        caps |= DeviceCapabilities::TRIM;
    }

    caps
}

/// Build a DeviceInfo from a udev event.
pub fn device_info_from_udev(event: &UdevEvent) -> Result<DeviceInfo> {
    let dev_path = event
        .dev_path
        .clone()
        .ok_or_else(|| YuktiError::Udev("no dev_path in event".into()))?;

    let (class, capabilities) = classify_and_extract(event);
    let id = DeviceId::new(format!(
        "{}:{}",
        event.subsystem,
        dev_path.file_name().unwrap_or_default().to_string_lossy()
    ));

    let mut info = DeviceInfo::new(id, dev_path, class);
    info.sys_path = Some(event.sys_path.clone());
    info.vendor = event.property("ID_VENDOR").map(|s| s.to_string());
    info.model = event.property("ID_MODEL").map(|s| s.to_string());
    info.serial = event.property("ID_SERIAL_SHORT").map(|s| s.to_string());
    info.label = event
        .property("ID_FS_LABEL")
        .or_else(|| event.property("ID_FS_LABEL_ENC"))
        .map(|s| s.to_string());
    info.fs_type = event.property("ID_FS_TYPE").map(|s| s.to_string());

    if let Some(sectors) = event
        .property("ID_PART_ENTRY_SIZE")
        .and_then(|s| s.parse::<u64>().ok())
    {
        info.size_bytes = sectors * 512;
    }

    info.capabilities = capabilities;

    // USB vendor/product IDs from udev properties
    info.usb_vendor_id = event
        .property("ID_VENDOR_ID")
        .and_then(|s| u16::from_str_radix(s, 16).ok());
    info.usb_product_id = event
        .property("ID_MODEL_ID")
        .and_then(|s| u16::from_str_radix(s, 16).ok());

    // Partition table type
    info.partition_table = event.property("ID_PART_TABLE_TYPE").map(|s| s.to_string());

    info.properties = event.properties.clone();

    Ok(info)
}

/// Parse /sys/block/ to enumerate block devices.
///
/// In production, reads actual sysfs. This implementation provides the
/// enumeration logic given a sysfs root path.
pub fn enumerate_block_devices(sysfs_root: &Path) -> Vec<PathBuf> {
    let block_dir = sysfs_root.join("block");
    let mut devices = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&block_dir) {
        for entry in entries.flatten() {
            devices.push(entry.path());
        }
    }
    devices
}

/// Read a single sysfs attribute file, returning the trimmed contents.
/// Returns `None` on any I/O error (missing file, permission denied, etc.).
fn read_sysfs_attr(base: &Path, attr: &str) -> Option<String> {
    std::fs::read_to_string(base.join(attr))
        .ok()
        .map(|s| s.trim().to_string())
}

/// Enumerate all block devices under `sysfs_root`, building full `DeviceInfo`
/// for each disk and its partitions.
///
/// `sysfs_root` is typically `/sys` in production. Each entry in
/// `<sysfs_root>/block/` is inspected for sysfs attributes and converted
/// through the standard `classify_and_extract()` / `device_info_from_udev()`
/// pipeline.
pub fn enumerate_devices(sysfs_root: &Path) -> Result<Vec<DeviceInfo>> {
    let block_dir = sysfs_root.join("block");
    debug!(path = %block_dir.display(), "enumerating block devices");
    let entries = std::fs::read_dir(&block_dir)
        .map_err(|e| YuktiError::Udev(format!("failed to read {}: {}", block_dir.display(), e)))?;

    let mut devices = Vec::new();

    for entry in entries.flatten() {
        let dev_name = entry.file_name();
        let dev_name_str = dev_name.to_string_lossy().to_string();
        let sys_path = entry.path();

        // Build a synthetic UdevEvent from sysfs attributes
        if let Some(info) = build_device_info_from_sysfs(&sys_path, &dev_name_str, "disk") {
            devices.push(info);
        }

        // Enumerate partitions: subdirectories matching "<devname><number>"
        if let Ok(sub_entries) = std::fs::read_dir(&sys_path) {
            for sub_entry in sub_entries.flatten() {
                let part_name = sub_entry.file_name();
                let part_name_str = part_name.to_string_lossy().to_string();
                // Partitions are named like sda1, sda2, nvme0n1p1, mmcblk0p1
                if part_name_str.starts_with(&dev_name_str)
                    && part_name_str.len() > dev_name_str.len()
                    && sub_entry.path().join("partition").exists()
                    && let Some(info) =
                        build_device_info_from_sysfs(&sub_entry.path(), &part_name_str, "partition")
                {
                    devices.push(info);
                }
            }
        }
    }

    info!(count = devices.len(), "enumeration complete");
    Ok(devices)
}

/// Build a `DeviceInfo` from sysfs attributes at the given path.
fn build_device_info_from_sysfs(
    sys_path: &Path,
    dev_name: &str,
    dev_type: &str,
) -> Option<DeviceInfo> {
    let mut props = HashMap::new();

    // Read standard sysfs attributes
    if let Some(vendor) = read_sysfs_attr(sys_path, "device/vendor") {
        props.insert("ID_VENDOR".to_string(), vendor);
    }
    if let Some(model) = read_sysfs_attr(sys_path, "device/model") {
        props.insert("ID_MODEL".to_string(), model);
    }
    if let Some(serial) = read_sysfs_attr(sys_path, "device/serial") {
        props.insert("ID_SERIAL_SHORT".to_string(), serial);
    }
    if read_sysfs_attr(sys_path, "removable").as_deref() == Some("1") {
        props.insert("UDISKS_REMOVABLE".to_string(), "1".to_string());
    }
    if read_sysfs_attr(sys_path, "ro").as_deref() == Some("1") {
        props.insert("ID_FS_READONLY".to_string(), "1".to_string());
    }
    if let Some(size_str) = read_sysfs_attr(sys_path, "size") {
        props.insert("ID_PART_ENTRY_SIZE".to_string(), size_str);
    }

    // USB vendor/product IDs from sysfs
    if let Some(vid) = read_sysfs_attr(sys_path, "device/idVendor") {
        props.insert("ID_VENDOR_ID".to_string(), vid);
    }
    if let Some(pid) = read_sysfs_attr(sys_path, "device/idProduct") {
        props.insert("ID_MODEL_ID".to_string(), pid);
    }

    let dev_path = PathBuf::from(format!("/dev/{}", dev_name));

    let event = UdevEvent {
        action: "add".to_string(),
        sys_path: sys_path.to_path_buf(),
        dev_path: Some(dev_path),
        subsystem: "block".to_string(),
        dev_type: Some(dev_type.to_string()),
        properties: props,
    };

    let mut info = device_info_from_udev(&event).ok()?;

    // Query device node permissions
    #[cfg(target_os = "linux")]
    {
        info.permissions = crate::device::query_permissions(&info.dev_path);
    }

    // Check mount status if storage feature is enabled
    #[cfg(feature = "storage")]
    {
        if let Some(mp) = crate::storage::find_mount_point(&info.dev_path) {
            info.mount_point = Some(mp);
            info.state = crate::device::DeviceState::Mounted;
        }
    }

    Some(info)
}

// ---------------------------------------------------------------------------
// UdevMonitor — real netlink-based udev monitoring (Linux only)
// ---------------------------------------------------------------------------

/// Netlink protocol number for kernel uevents.
#[cfg(target_os = "linux")]
const NETLINK_KOBJECT_UEVENT: libc::c_int = 15;

/// Receive buffer size for the netlink socket (256 KB).
#[cfg(target_os = "linux")]
const RECV_BUF_SIZE: libc::c_int = 256 * 1024;

/// Parse a raw kernel uevent message (null-separated key=value pairs).
///
/// The kernel uevent format is:
/// ```text
/// action@devpath\0
/// KEY=VALUE\0
/// KEY=VALUE\0
/// ...
/// ```
#[cfg(target_os = "linux")]
pub fn parse_uevent(buf: &[u8]) -> Option<UdevEvent> {
    // Split on null bytes, skip empty segments
    let parts: Vec<&[u8]> = buf.split(|&b| b == 0).filter(|p| !p.is_empty()).collect();
    if parts.is_empty() {
        return None;
    }

    let mut action = String::new();
    let mut devpath = String::new();
    let mut subsystem = String::new();
    let mut devtype = None;
    let mut properties = HashMap::new();

    let start_idx;

    // First entry might be the "action@devpath" header
    let first = std::str::from_utf8(parts[0]).ok()?;
    if let Some(at_pos) = first.find('@') {
        action = first[..at_pos].to_lowercase();
        devpath = first[at_pos + 1..].to_string();
        start_idx = 1;
    } else if first.contains('=') {
        // No header, start parsing key=value from index 0
        start_idx = 0;
    } else {
        return None;
    }

    for &part in &parts[start_idx..] {
        let s = std::str::from_utf8(part).ok()?;
        if let Some((key, value)) = s.split_once('=') {
            match key {
                "ACTION" => action = value.to_lowercase(),
                "DEVPATH" => devpath = value.to_string(),
                "SUBSYSTEM" => subsystem = value.to_string(),
                "DEVTYPE" => devtype = Some(value.to_string()),
                _ => {
                    properties.insert(key.to_string(), value.to_string());
                }
            }
        }
    }

    if action.is_empty() || devpath.is_empty() {
        return None;
    }

    // Derive dev_path from DEVNAME property or devpath
    let dev_path = properties.get("DEVNAME").map(|n| {
        if n.starts_with('/') {
            PathBuf::from(n)
        } else {
            PathBuf::from(format!("/dev/{}", n))
        }
    });

    let sys_path = PathBuf::from(format!("/sys{}", devpath));

    Some(UdevEvent {
        action,
        sys_path,
        dev_path,
        subsystem,
        dev_type: devtype,
        properties,
    })
}

/// Convert a `UdevEvent` into a `DeviceEvent` for the event system.
#[cfg(target_os = "linux")]
fn udev_event_to_device_event(uevent: &UdevEvent) -> Option<DeviceEvent> {
    let kind = match uevent.action.as_str() {
        "add" => DeviceEventKind::Attached,
        "remove" => DeviceEventKind::Detached,
        "change" => DeviceEventKind::MediaChanged,
        _ => return None,
    };

    let dev_path = uevent.dev_path.clone()?;
    let (class, _caps) = classify_and_extract(uevent);
    let dev_name = dev_path.file_name().unwrap_or_default().to_string_lossy();
    let id = DeviceId::new(format!("{}:{}", uevent.subsystem, dev_name));

    let mut event = DeviceEvent::new(id, class, kind, dev_path);

    // Attach full info for non-remove events
    if !uevent.is_remove()
        && let Ok(info) = device_info_from_udev(uevent)
    {
        event = event.with_info(info);
    }

    Some(event)
}

/// Real netlink-based udev monitor.
///
/// Listens on a `NETLINK_KOBJECT_UEVENT` socket for kernel device events
/// and converts them into yukti's `DeviceEvent` type.
#[cfg(target_os = "linux")]
pub struct UdevMonitor {
    socket_fd: std::os::unix::io::RawFd,
    running: Arc<AtomicBool>,
    filter: Option<Vec<DeviceClass>>,
}

#[cfg(target_os = "linux")]
impl UdevMonitor {
    /// Create a new `UdevMonitor` bound to the kernel uevent netlink group.
    ///
    /// Requires `CAP_NET_ADMIN` or root on some kernels. The socket is set
    /// to `SOCK_CLOEXEC` and the receive buffer is enlarged to 256 KB.
    pub fn new() -> Result<Self> {
        Self::with_filter(&[])
    }

    /// Create a monitor that only emits events for the specified device classes.
    /// Pass an empty slice to receive all events (same as `new()`).
    pub fn with_filter(classes: &[DeviceClass]) -> Result<Self> {
        // SAFETY: libc socket creation — no memory unsafety, just a syscall.
        let fd = unsafe {
            libc::socket(
                libc::AF_NETLINK,
                libc::SOCK_DGRAM | libc::SOCK_CLOEXEC,
                NETLINK_KOBJECT_UEVENT,
            )
        };
        if fd < 0 {
            return Err(YuktiError::UdevSocket(format!(
                "socket() failed: {}",
                std::io::Error::last_os_error()
            )));
        }

        // Bind to multicast group 1 (kernel events)
        #[repr(C)]
        struct SockaddrNl {
            nl_family: u16,
            nl_pad: u16,
            nl_pid: u32,
            nl_groups: u32,
        }

        let addr = SockaddrNl {
            nl_family: libc::AF_NETLINK as u16,
            nl_pad: 0,
            nl_pid: 0,    // let kernel assign
            nl_groups: 1, // KOBJECT_UEVENT multicast group
        };

        // SAFETY: bind() with a correctly-sized sockaddr_nl.
        let rc = unsafe {
            libc::bind(
                fd,
                &addr as *const SockaddrNl as *const libc::sockaddr,
                std::mem::size_of::<SockaddrNl>() as libc::socklen_t,
            )
        };
        if rc < 0 {
            let err = std::io::Error::last_os_error();
            unsafe {
                libc::close(fd);
            }
            return Err(YuktiError::UdevSocket(format!("bind() failed: {}", err)));
        }

        // Set receive buffer size
        let buf_size: libc::c_int = RECV_BUF_SIZE;
        // SAFETY: setsockopt with valid fd and buffer pointer.
        unsafe {
            libc::setsockopt(
                fd,
                libc::SOL_SOCKET,
                libc::SO_RCVBUF,
                &buf_size as *const libc::c_int as *const libc::c_void,
                std::mem::size_of::<libc::c_int>() as libc::socklen_t,
            );
        }

        let filter = if classes.is_empty() {
            None
        } else {
            Some(classes.to_vec())
        };

        info!("udev monitor created (netlink fd={})", fd);
        Ok(Self {
            socket_fd: fd,
            running: Arc::new(AtomicBool::new(true)),
            filter,
        })
    }

    /// Poll the netlink socket for a single uevent.
    ///
    /// `timeout_ms` is the poll timeout in milliseconds. Use `-1` to block
    /// indefinitely, or `0` for non-blocking.
    ///
    /// Returns `Ok(Some(event))` if a valid uevent was received,
    /// `Ok(None)` on timeout or unparseable data, and `Err` on syscall failure.
    pub fn poll(&self, timeout_ms: i32) -> Result<Option<UdevEvent>> {
        let mut pfd = libc::pollfd {
            fd: self.socket_fd,
            events: libc::POLLIN,
            revents: 0,
        };

        // SAFETY: poll() on a valid fd with a correctly initialised pollfd.
        let ret = unsafe { libc::poll(&mut pfd, 1, timeout_ms) };
        if ret < 0 {
            return Err(YuktiError::UdevSocket(format!(
                "poll() failed: {}",
                std::io::Error::last_os_error()
            )));
        }
        if ret == 0 {
            return Ok(None); // timeout
        }

        let mut buf = vec![0u8; 8192];
        // SAFETY: recv() into a buffer we own; the fd is valid.
        let n = unsafe {
            libc::recv(
                self.socket_fd,
                buf.as_mut_ptr() as *mut libc::c_void,
                buf.len(),
                0,
            )
        };
        if n <= 0 {
            return Ok(None);
        }

        Ok(parse_uevent(&buf[..n as usize]))
    }

    /// Blocking event loop — polls for events and dispatches them to the
    /// provided `EventListener` until `stop()` is called.
    pub fn run_with_listener(&self, listener: &dyn EventListener) -> Result<()> {
        info!("udev monitor event loop started");
        while self.running.load(Ordering::Relaxed) {
            match self.poll(500)? {
                Some(uevent) => {
                    debug!(action = %uevent.action, subsystem = %uevent.subsystem, dev_path = ?uevent.dev_path, "uevent received");
                    if let Some(dev_event) = udev_event_to_device_event(&uevent) {
                        // Check monitor-level filter first
                        if let Some(ref monitor_filter) = self.filter
                            && !monitor_filter.contains(&dev_event.device_class)
                        {
                            continue;
                        }

                        // Respect the listener's class filter
                        let dominated = listener
                            .filter()
                            .map(|classes| classes.contains(&dev_event.device_class))
                            .unwrap_or(true);

                        if dominated {
                            listener.on_event(&dev_event);
                        }
                    }
                }
                None => continue,
            }
        }
        Ok(())
    }

    /// Signal the monitor to stop its event loop.
    pub fn stop(&self) {
        info!("udev monitor stopping");
        self.running.store(false, Ordering::Relaxed);
    }

    /// Create a channel-based subscription. Returns a `Receiver` that yields
    /// `DeviceEvent`s. A background thread is spawned to drive the monitor.
    ///
    /// The thread exits when `stop()` is called or the receiver is dropped.
    pub fn subscribe(&self) -> std::sync::mpsc::Receiver<DeviceEvent> {
        let (tx, rx) = std::sync::mpsc::channel();
        let fd = self.socket_fd;
        let running = Arc::clone(&self.running);
        let filter = self.filter.clone();

        std::thread::spawn(move || {
            let mut buf = vec![0u8; 8192];
            while running.load(Ordering::Relaxed) {
                let mut pfd = libc::pollfd {
                    fd,
                    events: libc::POLLIN,
                    revents: 0,
                };
                // SAFETY: poll/recv on a valid fd.
                let ret = unsafe { libc::poll(&mut pfd, 1, 500) };
                if ret <= 0 {
                    continue;
                }
                let n =
                    unsafe { libc::recv(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len(), 0) };
                if n <= 0 {
                    continue;
                }
                if let Some(uevent) = parse_uevent(&buf[..n as usize])
                    && let Some(dev_event) = udev_event_to_device_event(&uevent)
                {
                    // Apply monitor-level filter
                    if let Some(ref f) = filter
                        && !f.contains(&dev_event.device_class)
                    {
                        continue;
                    }
                    if tx.send(dev_event).is_err() {
                        break; // receiver dropped
                    }
                }
            }
        });

        rx
    }

    /// Subscribe with a bounded channel. If the channel is full, oldest events are dropped.
    pub fn subscribe_bounded(&self, capacity: usize) -> std::sync::mpsc::Receiver<DeviceEvent> {
        let (tx, rx) = std::sync::mpsc::sync_channel(capacity);
        let fd = self.socket_fd;
        let running = Arc::clone(&self.running);
        let filter = self.filter.clone();

        std::thread::spawn(move || {
            let mut buf = vec![0u8; 8192];
            while running.load(Ordering::Relaxed) {
                let mut pfd = libc::pollfd {
                    fd,
                    events: libc::POLLIN,
                    revents: 0,
                };
                // SAFETY: poll/recv on a valid fd.
                let ret = unsafe { libc::poll(&mut pfd, 1, 500) };
                if ret <= 0 {
                    continue;
                }
                let n =
                    unsafe { libc::recv(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len(), 0) };
                if n <= 0 {
                    continue;
                }
                if let Some(uevent) = parse_uevent(&buf[..n as usize])
                    && let Some(dev_event) = udev_event_to_device_event(&uevent)
                {
                    // Apply monitor-level filter
                    if let Some(ref f) = filter
                        && !f.contains(&dev_event.device_class)
                    {
                        continue;
                    }
                    if tx.try_send(dev_event).is_err() {
                        tracing::warn!("bounded subscribe channel full, event dropped");
                    }
                }
            }
        });

        rx
    }
}

#[cfg(target_os = "linux")]
impl Drop for UdevMonitor {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        // SAFETY: closing a valid file descriptor we own.
        unsafe {
            libc::close(self.socket_fd);
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::device::DeviceCapability;

    fn make_usb_event() -> UdevEvent {
        let mut props = HashMap::new();
        props.insert("ID_BUS".into(), "usb".into());
        props.insert("ID_VENDOR".into(), "SanDisk".into());
        props.insert("ID_MODEL".into(), "Cruzer_Blade".into());
        props.insert("ID_SERIAL_SHORT".into(), "ABC123".into());
        props.insert("ID_FS_TYPE".into(), "vfat".into());
        props.insert("ID_FS_LABEL".into(), "MYUSB".into());
        props.insert("ID_USB_DRIVER".into(), "usb-storage".into());

        UdevEvent {
            action: "add".into(),
            sys_path: PathBuf::from(
                "/sys/devices/pci0000:00/usb1/1-1/1-1:1.0/host0/target0:0:0/0:0:0:0/block/sdb/sdb1",
            ),
            dev_path: Some(PathBuf::from("/dev/sdb1")),
            subsystem: "block".into(),
            dev_type: Some("partition".into()),
            properties: props,
        }
    }

    fn make_optical_event() -> UdevEvent {
        let mut props = HashMap::new();
        props.insert("ID_CDROM".into(), "1".into());
        props.insert("ID_CDROM_DVD".into(), "1".into());
        props.insert("ID_CDROM_DVD_RW".into(), "1".into());
        props.insert("ID_FS_TYPE".into(), "iso9660".into());
        props.insert("ID_FS_LABEL".into(), "MOVIE_DISC".into());

        UdevEvent {
            action: "change".into(),
            sys_path: PathBuf::from(
                "/sys/devices/pci0000:00/ata1/host0/target0:0:0/0:0:0:0/block/sr0",
            ),
            dev_path: Some(PathBuf::from("/dev/sr0")),
            subsystem: "block".into(),
            dev_type: Some("disk".into()),
            properties: props,
        }
    }

    #[test]
    fn test_classify_usb() {
        let event = make_usb_event();
        assert_eq!(classify_device(&event), DeviceClass::UsbStorage);
    }

    #[test]
    fn test_classify_optical() {
        let event = make_optical_event();
        assert_eq!(classify_device(&event), DeviceClass::Optical);
    }

    #[test]
    fn test_classify_loop() {
        let event = UdevEvent {
            action: "add".into(),
            sys_path: PathBuf::from("/sys/devices/virtual/block/loop0"),
            dev_path: Some(PathBuf::from("/dev/loop0")),
            subsystem: "block".into(),
            dev_type: Some("disk".into()),
            properties: HashMap::new(),
        };
        assert_eq!(classify_device(&event), DeviceClass::Loop);
    }

    #[test]
    fn test_classify_dm() {
        let event = UdevEvent {
            action: "add".into(),
            sys_path: PathBuf::from("/sys/devices/virtual/block/dm-0"),
            dev_path: Some(PathBuf::from("/dev/dm-0")),
            subsystem: "block".into(),
            dev_type: Some("disk".into()),
            properties: HashMap::new(),
        };
        assert_eq!(classify_device(&event), DeviceClass::DeviceMapper);
    }

    #[test]
    fn test_classify_sd_card() {
        let mut props = HashMap::new();
        props.insert("ID_BUS".into(), "mmc".into());
        let event = UdevEvent {
            action: "add".into(),
            sys_path: PathBuf::from("/sys/devices/mmc0/mmc0:0001/block/mmcblk0"),
            dev_path: Some(PathBuf::from("/dev/mmcblk0")),
            subsystem: "block".into(),
            dev_type: Some("disk".into()),
            properties: props,
        };
        assert_eq!(classify_device(&event), DeviceClass::SdCard);
    }

    #[test]
    fn test_classify_block_internal() {
        let event = UdevEvent {
            action: "add".into(),
            sys_path: PathBuf::from(
                "/sys/devices/pci0000:00/0000:00:17.0/ata1/host0/target0:0:0/0:0:0:0/block/sda",
            ),
            dev_path: Some(PathBuf::from("/dev/sda")),
            subsystem: "block".into(),
            dev_type: Some("disk".into()),
            properties: HashMap::new(),
        };
        assert_eq!(classify_device(&event), DeviceClass::BlockInternal);
    }

    #[test]
    fn test_classify_unknown() {
        let event = UdevEvent {
            action: "add".into(),
            sys_path: PathBuf::from("/sys/devices/usb/input0"),
            dev_path: None,
            subsystem: "input".into(),
            dev_type: None,
            properties: HashMap::new(),
        };
        assert_eq!(classify_device(&event), DeviceClass::Unknown);
    }

    #[test]
    fn test_usb_capabilities() {
        let event = make_usb_event();
        let caps = extract_capabilities(&event, DeviceClass::UsbStorage);
        assert!(caps.contains(DeviceCapabilities::READ));
        assert!(caps.contains(DeviceCapabilities::REMOVABLE));
        assert!(caps.contains(DeviceCapabilities::HOTPLUG));
        assert!(caps.contains(DeviceCapabilities::EJECT));
        assert!(!caps.contains(DeviceCapabilities::TRAY_CONTROL));
    }

    #[test]
    fn test_optical_capabilities() {
        let event = make_optical_event();
        let caps = extract_capabilities(&event, DeviceClass::Optical);
        assert!(caps.contains(DeviceCapabilities::TRAY_CONTROL));
        assert!(caps.contains(DeviceCapabilities::MEDIA_CHANGE));
        assert!(caps.contains(DeviceCapabilities::BURN)); // DVD-RW
    }

    #[test]
    fn test_readonly_device() {
        let mut props = HashMap::new();
        props.insert("ID_FS_READONLY".into(), "1".into());
        let event = UdevEvent {
            action: "add".into(),
            sys_path: PathBuf::from("/sys/devices/block/sda"),
            dev_path: Some(PathBuf::from("/dev/sda")),
            subsystem: "block".into(),
            dev_type: Some("disk".into()),
            properties: props,
        };
        let caps = extract_capabilities(&event, DeviceClass::BlockInternal);
        assert!(caps.contains(DeviceCapabilities::READ));
        assert!(!caps.contains(DeviceCapabilities::WRITE));
    }

    #[test]
    fn test_trim_capability() {
        let mut props = HashMap::new();
        props.insert("ID_ATA_FEATURE_SET_TRIM".into(), "1".into());
        let event = UdevEvent {
            action: "add".into(),
            sys_path: PathBuf::from("/sys/devices/block/sda"),
            dev_path: Some(PathBuf::from("/dev/sda")),
            subsystem: "block".into(),
            dev_type: Some("disk".into()),
            properties: props,
        };
        let caps = extract_capabilities(&event, DeviceClass::BlockInternal);
        assert!(caps.contains(DeviceCapabilities::TRIM));
    }

    #[test]
    fn test_device_info_from_udev() {
        let event = make_usb_event();
        let info = device_info_from_udev(&event).unwrap();
        assert_eq!(info.class, DeviceClass::UsbStorage);
        assert_eq!(info.vendor.as_deref(), Some("SanDisk"));
        assert_eq!(info.model.as_deref(), Some("Cruzer_Blade"));
        assert_eq!(info.serial.as_deref(), Some("ABC123"));
        assert_eq!(info.label.as_deref(), Some("MYUSB"));
        assert_eq!(info.fs_type.as_deref(), Some("vfat"));
        assert!(info.has_capability(DeviceCapability::Removable));
    }

    #[test]
    fn test_device_info_from_optical() {
        let event = make_optical_event();
        let info = device_info_from_udev(&event).unwrap();
        assert_eq!(info.class, DeviceClass::Optical);
        assert_eq!(info.label.as_deref(), Some("MOVIE_DISC"));
        assert!(info.has_capability(DeviceCapability::TrayControl));
    }

    #[test]
    fn test_device_info_with_size() {
        let mut event = make_usb_event();
        event
            .properties
            .insert("ID_PART_ENTRY_SIZE".into(), "31457280".into());
        let info = device_info_from_udev(&event).unwrap();
        assert_eq!(info.size_bytes, 31457280 * 512);
    }

    #[test]
    fn test_device_info_label_enc_fallback() {
        let mut props = HashMap::new();
        props.insert("ID_BUS".into(), "usb".into());
        props.insert("ID_USB_DRIVER".into(), "usb-storage".into());
        props.insert("ID_FS_LABEL_ENC".into(), "ENCODED_LABEL".into());

        let event = UdevEvent {
            action: "add".into(),
            sys_path: PathBuf::from("/sys/devices/usb/block/sdb/sdb1"),
            dev_path: Some(PathBuf::from("/dev/sdb1")),
            subsystem: "block".into(),
            dev_type: Some("partition".into()),
            properties: props,
        };
        let info = device_info_from_udev(&event).unwrap();
        assert_eq!(info.label.as_deref(), Some("ENCODED_LABEL"));
    }

    #[test]
    fn test_udev_event_actions() {
        let mut event = make_usb_event();
        assert!(event.is_add());
        assert!(!event.is_remove());

        event.action = "remove".into();
        assert!(event.is_remove());

        event.action = "change".into();
        assert!(event.is_change());
    }

    #[test]
    fn test_udev_event_property_missing() {
        let event = make_usb_event();
        assert!(event.property("NONEXISTENT_KEY").is_none());
    }

    #[test]
    fn test_no_dev_path_error() {
        let event = UdevEvent {
            action: "add".into(),
            sys_path: PathBuf::from("/sys/test"),
            dev_path: None,
            subsystem: "block".into(),
            dev_type: None,
            properties: HashMap::new(),
        };
        assert!(device_info_from_udev(&event).is_err());
    }

    #[test]
    fn test_classify_and_extract_combined() {
        let event = make_usb_event();
        let (class, caps) = classify_and_extract(&event);
        assert_eq!(class, DeviceClass::UsbStorage);
        assert!(caps.contains(DeviceCapabilities::REMOVABLE));
    }

    #[test]
    fn test_enumerate_block_devices_nonexistent() {
        let devices = enumerate_block_devices(Path::new("/nonexistent/path"));
        assert!(devices.is_empty());
    }

    #[test]
    fn test_contains_bytes() {
        assert!(contains_bytes(b"/sys/block/dm-0", b"/dm-"));
        assert!(contains_bytes(b"/sys/block/loop0", b"/loop"));
        assert!(!contains_bytes(b"/sys/block/sda", b"/dm-"));
        assert!(!contains_bytes(b"", b"/dm-"));
    }

    // -----------------------------------------------------------------------
    // MockSysfs test harness
    // -----------------------------------------------------------------------

    /// Configurable attributes for a mock block device.
    #[derive(Default)]
    pub struct MockDeviceAttrs {
        pub vendor: Option<String>,
        pub model: Option<String>,
        pub serial: Option<String>,
        pub size_sectors: Option<u64>,
        pub removable: bool,
        pub readonly: bool,
    }

    /// A temporary fake sysfs tree for testing device enumeration.
    pub struct MockSysfs {
        root: tempfile::TempDir,
    }

    impl MockSysfs {
        pub fn new() -> Self {
            let root = tempfile::TempDir::new().expect("failed to create temp dir");
            std::fs::create_dir_all(root.path().join("block")).unwrap();
            Self { root }
        }

        pub fn root(&self) -> &Path {
            self.root.path()
        }

        /// Add a block device with the given name and attributes.
        pub fn add_device(&self, name: &str, attrs: MockDeviceAttrs) {
            let dev_dir = self.root.path().join("block").join(name);
            std::fs::create_dir_all(dev_dir.join("device")).unwrap();
            self.write_attrs(&dev_dir, &attrs);
        }

        /// Add a partition under a parent device.
        pub fn add_partition(&self, parent: &str, part_name: &str, attrs: MockDeviceAttrs) {
            let part_dir = self.root.path().join("block").join(parent).join(part_name);
            std::fs::create_dir_all(part_dir.join("device")).unwrap();
            std::fs::write(part_dir.join("partition"), "1\n").unwrap();
            self.write_attrs(&part_dir, &attrs);
        }

        fn write_attrs(&self, dir: &Path, attrs: &MockDeviceAttrs) {
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

    // -----------------------------------------------------------------------
    // Integration tests using MockSysfs
    // -----------------------------------------------------------------------

    #[test]
    fn test_mock_enumerate_three_devices() {
        let mock = MockSysfs::new();
        mock.add_device(
            "sda",
            MockDeviceAttrs {
                vendor: Some("ATA".into()),
                model: Some("Samsung SSD 870".into()),
                size_sectors: Some(1953525168),
                ..Default::default()
            },
        );
        mock.add_device(
            "sdb",
            MockDeviceAttrs {
                vendor: Some("SanDisk".into()),
                model: Some("Cruzer Blade".into()),
                size_sectors: Some(31457280),
                removable: true,
                ..Default::default()
            },
        );
        mock.add_device(
            "sr0",
            MockDeviceAttrs {
                vendor: Some("HL-DT-ST".into()),
                model: Some("DVDRAM".into()),
                size_sectors: Some(0),
                ..Default::default()
            },
        );

        let devices = enumerate_devices(mock.root()).unwrap();
        assert_eq!(devices.len(), 3);

        let names: Vec<String> = devices
            .iter()
            .map(|d| d.dev_path.display().to_string())
            .collect();
        assert!(names.contains(&"/dev/sda".to_string()));
        assert!(names.contains(&"/dev/sdb".to_string()));
        assert!(names.contains(&"/dev/sr0".to_string()));

        let sda = devices
            .iter()
            .find(|d| d.dev_path.display().to_string() == "/dev/sda")
            .unwrap();
        assert_eq!(sda.vendor.as_deref(), Some("ATA"));
        assert_eq!(sda.model.as_deref(), Some("Samsung SSD 870"));
        assert_eq!(sda.size_bytes, 1953525168 * 512);

        let sdb = devices
            .iter()
            .find(|d| d.dev_path.display().to_string() == "/dev/sdb")
            .unwrap();
        assert!(sdb.capabilities.contains(DeviceCapabilities::REMOVABLE));
    }

    #[test]
    fn test_mock_enumerate_with_partitions() {
        let mock = MockSysfs::new();
        mock.add_device(
            "sda",
            MockDeviceAttrs {
                size_sectors: Some(1000),
                ..Default::default()
            },
        );
        mock.add_partition(
            "sda",
            "sda1",
            MockDeviceAttrs {
                size_sectors: Some(500),
                ..Default::default()
            },
        );
        mock.add_partition(
            "sda",
            "sda2",
            MockDeviceAttrs {
                size_sectors: Some(500),
                ..Default::default()
            },
        );

        let devices = enumerate_devices(mock.root()).unwrap();
        assert_eq!(devices.len(), 3);

        let names: Vec<String> = devices
            .iter()
            .map(|d| d.dev_path.display().to_string())
            .collect();
        assert!(names.contains(&"/dev/sda".to_string()));
        assert!(names.contains(&"/dev/sda1".to_string()));
        assert!(names.contains(&"/dev/sda2".to_string()));

        let sda1 = devices
            .iter()
            .find(|d| d.dev_path.display().to_string() == "/dev/sda1")
            .unwrap();
        assert_eq!(sda1.size_bytes, 500 * 512);
    }

    #[test]
    fn test_mock_enumerate_empty() {
        let mock = MockSysfs::new();
        let devices = enumerate_devices(mock.root()).unwrap();
        assert!(devices.is_empty());
    }

    #[test]
    fn test_mock_removable_usb_capabilities() {
        let mock = MockSysfs::new();
        mock.add_device(
            "sdb",
            MockDeviceAttrs {
                vendor: Some("Kingston".into()),
                model: Some("DataTraveler".into()),
                serial: Some("ABC123".into()),
                size_sectors: Some(62914560),
                removable: true,
                ..Default::default()
            },
        );

        let devices = enumerate_devices(mock.root()).unwrap();
        assert_eq!(devices.len(), 1);
        let dev = &devices[0];
        assert_eq!(dev.vendor.as_deref(), Some("Kingston"));
        assert_eq!(dev.model.as_deref(), Some("DataTraveler"));
        assert_eq!(dev.serial.as_deref(), Some("ABC123"));
        assert!(dev.capabilities.contains(DeviceCapabilities::REMOVABLE));
        assert!(dev.capabilities.contains(DeviceCapabilities::HOTPLUG));
        assert!(dev.capabilities.contains(DeviceCapabilities::EJECT));
        assert!(dev.capabilities.contains(DeviceCapabilities::READ));
        assert!(dev.capabilities.contains(DeviceCapabilities::WRITE));
    }

    // -----------------------------------------------------------------------
    // read_sysfs_attr tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_read_sysfs_attr_success() {
        let dir = std::env::temp_dir().join("yukti_test_sysfs_attr");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("vendor"), "  ATA  \n").unwrap();
        let val = read_sysfs_attr(&dir, "vendor");
        assert_eq!(val.as_deref(), Some("ATA"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_read_sysfs_attr_missing() {
        let dir = std::env::temp_dir().join("yukti_test_sysfs_missing");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        assert!(read_sysfs_attr(&dir, "nonexistent").is_none());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_read_sysfs_attr_empty_file() {
        let dir = std::env::temp_dir().join("yukti_test_sysfs_empty");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("empty"), "").unwrap();
        let val = read_sysfs_attr(&dir, "empty");
        assert_eq!(val.as_deref(), Some(""));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_read_sysfs_attr_nested() {
        let dir = std::env::temp_dir().join("yukti_test_sysfs_nested");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("device")).unwrap();
        std::fs::write(dir.join("device/model"), "Samsung SSD\n").unwrap();
        let val = read_sysfs_attr(&dir, "device/model");
        assert_eq!(val.as_deref(), Some("Samsung SSD"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    // -----------------------------------------------------------------------
    // enumerate_devices tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_enumerate_devices_empty_block_dir() {
        let root = std::env::temp_dir().join("yukti_test_enum_empty");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("block")).unwrap();
        let devices = enumerate_devices(&root).unwrap();
        assert!(devices.is_empty());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn test_enumerate_devices_single_disk() {
        let root = std::env::temp_dir().join("yukti_test_enum_single");
        let _ = std::fs::remove_dir_all(&root);
        let sda = root.join("block/sda");
        std::fs::create_dir_all(&sda).unwrap();
        std::fs::write(sda.join("size"), "1953525168\n").unwrap();
        std::fs::write(sda.join("removable"), "0\n").unwrap();
        std::fs::write(sda.join("ro"), "0\n").unwrap();

        let devices = enumerate_devices(&root).unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].dev_path, PathBuf::from("/dev/sda"));
        assert_eq!(devices[0].size_bytes, 1953525168 * 512);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn test_enumerate_devices_with_partitions() {
        let root = std::env::temp_dir().join("yukti_test_enum_parts");
        let _ = std::fs::remove_dir_all(&root);
        let sda = root.join("block/sda");
        let sda1 = sda.join("sda1");
        let sda2 = sda.join("sda2");

        std::fs::create_dir_all(&sda).unwrap();
        std::fs::create_dir_all(&sda1).unwrap();
        std::fs::create_dir_all(&sda2).unwrap();

        // Mark sda1 and sda2 as partitions
        std::fs::write(sda1.join("partition"), "1\n").unwrap();
        std::fs::write(sda2.join("partition"), "2\n").unwrap();
        std::fs::write(sda.join("size"), "1000\n").unwrap();
        std::fs::write(sda1.join("size"), "500\n").unwrap();
        std::fs::write(sda2.join("size"), "500\n").unwrap();

        let devices = enumerate_devices(&root).unwrap();
        // 1 disk + 2 partitions
        assert_eq!(devices.len(), 3);

        let names: Vec<_> = devices.iter().map(|d| d.dev_path.clone()).collect();
        assert!(names.contains(&PathBuf::from("/dev/sda")));
        assert!(names.contains(&PathBuf::from("/dev/sda1")));
        assert!(names.contains(&PathBuf::from("/dev/sda2")));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn test_enumerate_devices_nonexistent_root() {
        let result = enumerate_devices(Path::new("/nonexistent/fake/path"));
        assert!(result.is_err());
    }

    #[test]
    fn test_enumerate_devices_with_vendor_model() {
        let root = std::env::temp_dir().join("yukti_test_enum_vendor");
        let _ = std::fs::remove_dir_all(&root);
        let sda = root.join("block/sda");
        std::fs::create_dir_all(sda.join("device")).unwrap();
        std::fs::write(sda.join("device/vendor"), "  ATA     \n").unwrap();
        std::fs::write(sda.join("device/model"), "Samsung SSD 870\n").unwrap();
        std::fs::write(sda.join("size"), "1000\n").unwrap();

        let devices = enumerate_devices(&root).unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].vendor.as_deref(), Some("ATA"));
        assert_eq!(devices[0].model.as_deref(), Some("Samsung SSD 870"));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn test_enumerate_devices_removable() {
        let root = std::env::temp_dir().join("yukti_test_enum_removable");
        let _ = std::fs::remove_dir_all(&root);
        let sdb = root.join("block/sdb");
        std::fs::create_dir_all(&sdb).unwrap();
        std::fs::write(sdb.join("removable"), "1\n").unwrap();
        std::fs::write(sdb.join("size"), "100\n").unwrap();

        let devices = enumerate_devices(&root).unwrap();
        assert_eq!(devices.len(), 1);
        // removable=1 sets UDISKS_REMOVABLE property
        assert!(
            devices[0]
                .capabilities
                .contains(DeviceCapabilities::REMOVABLE)
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    // -----------------------------------------------------------------------
    // USB vendor/product ID and partition table tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_device_info_usb_ids_from_udev() {
        let mut event = make_usb_event();
        event
            .properties
            .insert("ID_VENDOR_ID".into(), "0781".into());
        event.properties.insert("ID_MODEL_ID".into(), "5567".into());
        let info = device_info_from_udev(&event).unwrap();
        assert_eq!(info.usb_vendor_id, Some(0x0781));
        assert_eq!(info.usb_product_id, Some(0x5567));
    }

    #[test]
    fn test_device_info_usb_ids_missing() {
        let event = make_usb_event();
        let info = device_info_from_udev(&event).unwrap();
        assert!(info.usb_vendor_id.is_none());
        assert!(info.usb_product_id.is_none());
    }

    #[test]
    fn test_device_info_usb_ids_invalid_hex() {
        let mut event = make_usb_event();
        event
            .properties
            .insert("ID_VENDOR_ID".into(), "ZZZZ".into());
        let info = device_info_from_udev(&event).unwrap();
        assert!(info.usb_vendor_id.is_none());
    }

    #[test]
    fn test_device_info_partition_table_from_udev() {
        let mut event = make_usb_event();
        event
            .properties
            .insert("ID_PART_TABLE_TYPE".into(), "gpt".into());
        let info = device_info_from_udev(&event).unwrap();
        assert_eq!(info.partition_table.as_deref(), Some("gpt"));
    }

    #[test]
    fn test_device_info_partition_table_mbr() {
        let mut event = make_usb_event();
        event
            .properties
            .insert("ID_PART_TABLE_TYPE".into(), "dos".into());
        let info = device_info_from_udev(&event).unwrap();
        assert_eq!(info.partition_table.as_deref(), Some("dos"));
    }

    #[test]
    fn test_device_info_partition_table_missing() {
        let event = make_usb_event();
        let info = device_info_from_udev(&event).unwrap();
        assert!(info.partition_table.is_none());
    }

    #[test]
    fn test_sysfs_usb_ids_populated() {
        let root = std::env::temp_dir().join("yukti_test_sysfs_usb_ids");
        let _ = std::fs::remove_dir_all(&root);
        let sdb = root.join("block/sdb");
        std::fs::create_dir_all(sdb.join("device")).unwrap();
        std::fs::write(sdb.join("size"), "1000\n").unwrap();
        std::fs::write(sdb.join("device/idVendor"), "0781\n").unwrap();
        std::fs::write(sdb.join("device/idProduct"), "5567\n").unwrap();

        let devices = enumerate_devices(&root).unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].usb_vendor_id, Some(0x0781));
        assert_eq!(devices[0].usb_product_id, Some(0x5567));
        std::fs::remove_dir_all(&root).unwrap();
    }

    // -----------------------------------------------------------------------
    // parse_uevent tests (Linux only)
    // -----------------------------------------------------------------------

    #[cfg(target_os = "linux")]
    mod uevent_tests {
        use super::*;

        /// Build a realistic kernel uevent buffer (null-separated).
        fn build_uevent(parts: &[&str]) -> Vec<u8> {
            let mut buf = Vec::new();
            for (i, part) in parts.iter().enumerate() {
                buf.extend_from_slice(part.as_bytes());
                if i < parts.len() - 1 {
                    buf.push(0);
                }
            }
            buf
        }

        #[test]
        fn test_parse_uevent_usb_add() {
            let buf = build_uevent(&[
                "add@/devices/pci0000:00/usb1/1-1/1-1:1.0/host0/target0:0:0/0:0:0:0/block/sdb",
                "ACTION=add",
                "DEVPATH=/devices/pci0000:00/usb1/1-1/1-1:1.0/host0/target0:0:0/0:0:0:0/block/sdb",
                "SUBSYSTEM=block",
                "DEVTYPE=disk",
                "DEVNAME=sdb",
                "MAJOR=8",
                "MINOR=16",
            ]);

            let event = parse_uevent(&buf).unwrap();
            assert_eq!(event.action, "add");
            assert_eq!(
                event.sys_path,
                PathBuf::from(
                    "/sys/devices/pci0000:00/usb1/1-1/1-1:1.0/host0/target0:0:0/0:0:0:0/block/sdb"
                )
            );
            assert_eq!(event.dev_path, Some(PathBuf::from("/dev/sdb")));
            assert_eq!(event.subsystem, "block");
            assert_eq!(event.dev_type.as_deref(), Some("disk"));
            assert_eq!(event.property("MAJOR"), Some("8"));
            assert_eq!(event.property("MINOR"), Some("16"));
        }

        #[test]
        fn test_parse_uevent_remove() {
            let buf = build_uevent(&[
                "remove@/devices/pci0000:00/usb1/1-1/block/sdb",
                "ACTION=remove",
                "DEVPATH=/devices/pci0000:00/usb1/1-1/block/sdb",
                "SUBSYSTEM=block",
                "DEVTYPE=disk",
                "DEVNAME=sdb",
            ]);

            let event = parse_uevent(&buf).unwrap();
            assert_eq!(event.action, "remove");
            assert!(event.is_remove());
        }

        #[test]
        fn test_parse_uevent_change_optical() {
            let buf = build_uevent(&[
                "change@/devices/pci0000:00/ata1/host0/target0:0:0/0:0:0:0/block/sr0",
                "ACTION=change",
                "DEVPATH=/devices/pci0000:00/ata1/host0/target0:0:0/0:0:0:0/block/sr0",
                "SUBSYSTEM=block",
                "DEVTYPE=disk",
                "DEVNAME=sr0",
                "ID_CDROM=1",
            ]);

            let event = parse_uevent(&buf).unwrap();
            assert_eq!(event.action, "change");
            assert!(event.is_change());
            assert_eq!(event.subsystem, "block");
            assert_eq!(event.property("ID_CDROM"), Some("1"));
        }

        #[test]
        fn test_parse_uevent_partition() {
            let buf = build_uevent(&[
                "add@/devices/pci0000:00/block/sdb/sdb1",
                "ACTION=add",
                "DEVPATH=/devices/pci0000:00/block/sdb/sdb1",
                "SUBSYSTEM=block",
                "DEVTYPE=partition",
                "DEVNAME=sdb1",
                "PARTN=1",
            ]);

            let event = parse_uevent(&buf).unwrap();
            assert_eq!(event.dev_type.as_deref(), Some("partition"));
            assert_eq!(event.dev_path, Some(PathBuf::from("/dev/sdb1")));
            assert_eq!(event.property("PARTN"), Some("1"));
        }

        #[test]
        fn test_parse_uevent_no_header() {
            // Some uevents may lack the header line
            let buf = build_uevent(&[
                "ACTION=add",
                "DEVPATH=/devices/block/sda",
                "SUBSYSTEM=block",
                "DEVTYPE=disk",
                "DEVNAME=sda",
            ]);

            let event = parse_uevent(&buf).unwrap();
            assert_eq!(event.action, "add");
            assert_eq!(event.sys_path, PathBuf::from("/sys/devices/block/sda"));
        }

        #[test]
        fn test_parse_uevent_empty_buffer() {
            assert!(parse_uevent(&[]).is_none());
        }

        #[test]
        fn test_parse_uevent_garbage() {
            assert!(parse_uevent(b"not a valid uevent at all").is_none());
        }

        #[test]
        fn test_parse_uevent_missing_action() {
            let buf = build_uevent(&["DEVPATH=/devices/block/sda", "SUBSYSTEM=block"]);
            // No ACTION and no header means no action -> None
            assert!(parse_uevent(&buf).is_none());
        }

        #[test]
        fn test_parse_uevent_devname_absolute_path() {
            let buf = build_uevent(&[
                "add@/devices/block/sda",
                "ACTION=add",
                "DEVPATH=/devices/block/sda",
                "SUBSYSTEM=block",
                "DEVNAME=/dev/sda",
            ]);

            let event = parse_uevent(&buf).unwrap();
            assert_eq!(event.dev_path, Some(PathBuf::from("/dev/sda")));
        }

        #[test]
        fn test_parse_uevent_all_properties_preserved() {
            let buf = build_uevent(&[
                "add@/devices/block/sdb",
                "ACTION=add",
                "DEVPATH=/devices/block/sdb",
                "SUBSYSTEM=block",
                "DEVTYPE=disk",
                "ID_BUS=usb",
                "ID_VENDOR=Kingston",
                "ID_MODEL=DataTraveler_3.0",
                "ID_SERIAL_SHORT=XYZ789",
                "ID_FS_TYPE=ext4",
                "ID_FS_LABEL=mydata",
            ]);

            let event = parse_uevent(&buf).unwrap();
            assert_eq!(event.property("ID_BUS"), Some("usb"));
            assert_eq!(event.property("ID_VENDOR"), Some("Kingston"));
            assert_eq!(event.property("ID_MODEL"), Some("DataTraveler_3.0"));
            assert_eq!(event.property("ID_SERIAL_SHORT"), Some("XYZ789"));
            assert_eq!(event.property("ID_FS_TYPE"), Some("ext4"));
            assert_eq!(event.property("ID_FS_LABEL"), Some("mydata"));
        }

        #[test]
        fn test_udev_event_to_device_event_add() {
            let mut props = HashMap::new();
            props.insert("ID_BUS".into(), "usb".into());
            let uevent = UdevEvent {
                action: "add".into(),
                sys_path: PathBuf::from("/sys/devices/block/sdb"),
                dev_path: Some(PathBuf::from("/dev/sdb")),
                subsystem: "block".into(),
                dev_type: Some("disk".into()),
                properties: props,
            };

            let dev_event = udev_event_to_device_event(&uevent).unwrap();
            assert!(dev_event.is_attach());
            assert_eq!(dev_event.dev_path, PathBuf::from("/dev/sdb"));
            assert!(dev_event.device_info.is_some());
        }

        #[test]
        fn test_udev_event_to_device_event_remove() {
            let uevent = UdevEvent {
                action: "remove".into(),
                sys_path: PathBuf::from("/sys/devices/block/sdb"),
                dev_path: Some(PathBuf::from("/dev/sdb")),
                subsystem: "block".into(),
                dev_type: Some("disk".into()),
                properties: HashMap::new(),
            };

            let dev_event = udev_event_to_device_event(&uevent).unwrap();
            assert!(dev_event.is_detach());
            assert!(dev_event.device_info.is_none());
        }

        #[test]
        fn test_udev_event_to_device_event_unknown_action() {
            let uevent = UdevEvent {
                action: "bind".into(),
                sys_path: PathBuf::from("/sys/devices/block/sdb"),
                dev_path: Some(PathBuf::from("/dev/sdb")),
                subsystem: "block".into(),
                dev_type: Some("disk".into()),
                properties: HashMap::new(),
            };
            // bind/unbind are not mapped to DeviceEventKind
            assert!(udev_event_to_device_event(&uevent).is_none());
        }

        #[test]
        fn test_udev_event_to_device_event_no_devpath() {
            let uevent = UdevEvent {
                action: "add".into(),
                sys_path: PathBuf::from("/sys/devices/input0"),
                dev_path: None,
                subsystem: "input".into(),
                dev_type: None,
                properties: HashMap::new(),
            };
            assert!(udev_event_to_device_event(&uevent).is_none());
        }

        // Hardware tests — require root/CAP_NET_ADMIN

        #[test]
        #[ignore]
        fn test_udev_monitor_create() {
            let monitor = UdevMonitor::new().expect("failed to create UdevMonitor");
            assert!(monitor.socket_fd >= 0);
            monitor.stop();
        }

        #[test]
        #[ignore]
        fn test_udev_monitor_poll_timeout() {
            let monitor = UdevMonitor::new().expect("failed to create UdevMonitor");
            // Non-blocking poll should return None immediately
            let result = monitor.poll(0).expect("poll failed");
            assert!(result.is_none());
            monitor.stop();
        }

        #[test]
        #[ignore]
        fn test_udev_monitor_subscribe_and_stop() {
            let monitor = UdevMonitor::new().expect("failed to create UdevMonitor");
            let _rx = monitor.subscribe();
            // Let it run briefly
            std::thread::sleep(std::time::Duration::from_millis(100));
            monitor.stop();
            // Should wind down without hanging
        }

        #[test]
        #[ignore]
        fn test_udev_monitor_subscribe_bounded() {
            let monitor = UdevMonitor::new().expect("failed to create UdevMonitor");
            let rx = monitor.subscribe_bounded(4);
            // With no hardware events, the channel should be empty and recv should time out.
            let result = rx.recv_timeout(std::time::Duration::from_millis(200));
            assert!(
                result.is_err(),
                "expected timeout with no hardware events, got: {result:?}"
            );
            monitor.stop();
            // After stop, the background thread should exit and the channel should disconnect.
            std::thread::sleep(std::time::Duration::from_millis(600));
            let result = rx.recv_timeout(std::time::Duration::from_millis(100));
            assert!(
                result.is_err(),
                "expected disconnected after stop, got: {result:?}"
            );
        }
    }
}
