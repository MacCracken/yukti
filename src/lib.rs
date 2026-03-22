//! Yantra — Device abstraction layer for AGNOS
//!
//! Sanskrit: यन्त्र (yantra) — device, instrument, machine
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

pub use device::{Device, DeviceCapability, DeviceClass, DeviceId, DeviceInfo, DeviceState};
pub use error::YantraError;
pub use event::{DeviceEvent, DeviceEventKind, EventListener};
