//! Example: device detection, filesystem parsing, and disc type detection.
//!
//! Run with: cargo run --example detect --all-features

use std::collections::HashMap;
use std::path::PathBuf;

use yukti::device::{DeviceCapability, DeviceClass, DeviceId, DeviceInfo};
use yukti::optical::{DiscType, detect_disc_type};
use yukti::storage::{Filesystem, default_mount_point, validate_mount_point};
use yukti::udev::{UdevEvent, classify_device, device_info_from_udev};

fn main() {
    println!("=== Yukti Device Detection Example ===\n");

    // --- 1. Create a DeviceInfo manually ---
    println!("--- Manual DeviceInfo ---");
    let mut usb = DeviceInfo::new(
        DeviceId::new("block:sdb1"),
        PathBuf::from("/dev/sdb1"),
        DeviceClass::UsbStorage,
    );
    usb.label = Some("MY_FLASH".into());
    usb.vendor = Some("SanDisk".into());
    usb.model = Some("Cruzer Blade".into());
    usb.size_bytes = 16 * 1024 * 1024 * 1024; // 16 GB
    usb.capabilities |= DeviceCapability::Read.flag();
    usb.capabilities |= DeviceCapability::Write.flag();
    usb.capabilities |= DeviceCapability::Eject.flag();
    usb.capabilities |= DeviceCapability::Removable.flag();

    println!("  Device:       {}", usb.display_name());
    println!("  Class:        {}", usb.class);
    println!("  Size:         {}", usb.size_display());
    println!("  Removable:    {}", usb.is_removable());
    println!(
        "  Has Eject:    {}",
        usb.has_capability(DeviceCapability::Eject)
    );
    println!("  Capabilities: {:?}", usb.capabilities.to_vec());
    println!();

    // --- 2. Filesystem parsing ---
    println!("--- Filesystem Detection ---");
    let fs_types = ["ext4", "vfat", "iso9660", "ntfs", "btrfs", "zfs"];
    for name in &fs_types {
        let fs = Filesystem::from_str_type(name);
        println!(
            "  {name:10} -> {fs}  (writable: {}, optical: {})",
            fs.is_writable(),
            fs.is_optical_media()
        );
    }
    println!();

    // --- 3. Mount point helpers ---
    println!("--- Mount Point Helpers ---");
    let mount = default_mount_point(&usb);
    println!(
        "  Default mount for '{}': {}",
        usb.display_name(),
        mount.display()
    );

    let safe = PathBuf::from("/mnt/usb");
    let forbidden = PathBuf::from("/usr");
    println!(
        "  Validate /mnt/usb: {}",
        if validate_mount_point(&safe).is_ok() {
            "OK"
        } else {
            "REJECTED"
        }
    );
    println!(
        "  Validate /usr:     {}",
        if validate_mount_point(&forbidden).is_ok() {
            "OK"
        } else {
            "REJECTED"
        }
    );
    println!();

    // --- 4. Optical disc type detection ---
    println!("--- Disc Type Detection ---");
    let cases: &[(&str, bool, bool)] = &[
        ("cd", true, false),
        ("cd-rom", false, true),
        ("cd", true, true),
        ("dvd-rom", false, true),
        ("dvd-r", false, true),
        ("blu-ray", false, true),
        ("blank", false, false),
    ];
    for &(media, audio, data) in cases {
        let disc: DiscType = detect_disc_type(media, audio, data);
        println!(
            "  media={media:10} audio={audio:<5} data={data:<5} -> {disc} (has_audio={}, has_data={}, writable={})",
            disc.has_audio(),
            disc.has_data(),
            disc.is_writable()
        );
    }
    println!();

    // --- 5. udev classification from a synthetic event ---
    println!("--- Udev Device Classification ---");
    let mut props = HashMap::new();
    props.insert("ID_BUS".into(), "usb".into());
    props.insert("ID_VENDOR".into(), "Kingston".into());
    props.insert("ID_MODEL".into(), "DataTraveler_3.0".into());
    props.insert("ID_SERIAL_SHORT".into(), "XYZ789".into());
    props.insert("ID_FS_TYPE".into(), "ext4".into());
    props.insert("ID_FS_LABEL".into(), "BACKUP".into());
    props.insert("ID_USB_DRIVER".into(), "usb-storage".into());
    props.insert("ID_PART_ENTRY_SIZE".into(), "62914560".into());

    let event = UdevEvent {
        action: "add".into(),
        sys_path: PathBuf::from(
            "/sys/devices/pci0000:00/usb2/2-1/2-1:1.0/host1/target1:0:0/1:0:0:0/block/sdc/sdc1",
        ),
        dev_path: Some(PathBuf::from("/dev/sdc1")),
        subsystem: "block".into(),
        dev_type: Some("partition".into()),
        properties: props,
    };

    let class = classify_device(&event);
    println!("  Classified as: {class}");

    let info = device_info_from_udev(&event).expect("failed to build DeviceInfo from udev event");
    println!("  Device ID:     {}", info.id);
    println!(
        "  Vendor:        {}",
        info.vendor.as_deref().unwrap_or("n/a")
    );
    println!(
        "  Model:         {}",
        info.model.as_deref().unwrap_or("n/a")
    );
    println!(
        "  Label:         {}",
        info.label.as_deref().unwrap_or("n/a")
    );
    println!(
        "  FS type:       {}",
        info.fs_type.as_deref().unwrap_or("n/a")
    );
    println!("  Size:          {}", info.size_display());
    println!("  Capabilities:  {:?}", info.capabilities.to_vec());
    println!();

    println!("Done.");
}
