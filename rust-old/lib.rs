//! Yukti — Device abstraction layer for AGNOS
//!
//! Provides a unified API for detecting, monitoring, and managing hardware
//! devices: USB storage, optical drives, block devices, and udev hotplug events.
//!
//! # Modules
//!
//! - [`device`] — Core device types, traits, and capabilities
//! - [`event`] — Device events (attach, detach, media change)
//! - [`storage`] — USB/block device mount, eject, filesystem detection
//! - [`optical`] — Disc type detection, tray control, TOC reading
//! - [`udev`] — udev monitor, device enumeration, hotplug
//! - [`linux`] — Linux device manager (ties everything together)
//! - [`error`] — Error types

pub mod device;
pub mod error;
pub mod event;

#[cfg(feature = "optical")]
pub mod optical;

#[cfg(feature = "storage")]
pub mod storage;

#[cfg(feature = "udev")]
pub mod udev;

pub mod udev_rules;

#[cfg(target_os = "linux")]
pub mod linux;

pub use device::{
    Device, DeviceCapabilities, DeviceCapability, DeviceClass, DeviceHealth, DeviceId, DeviceInfo,
    DevicePermissions, DeviceState,
};
pub use error::YuktiError;
pub use event::{DeviceEvent, DeviceEventKind, EventListener};

#[cfg(target_os = "linux")]
pub use linux::LinuxDeviceManager;
