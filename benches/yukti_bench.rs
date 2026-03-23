use std::collections::HashMap;
use std::path::PathBuf;

use criterion::{Criterion, black_box, criterion_group, criterion_main};

use yukti::device::{
    DeviceCapabilities, DeviceCapability, DeviceClass, DeviceId, DeviceInfo, query_device_health,
    query_permissions,
};
use yukti::event::{DeviceEvent, DeviceEventKind, EventCollector, EventListener};
use yukti::optical::{DiscToc, DiscType, TocEntry, TrackType, detect_disc_type, is_dvd_video};
use yukti::storage::{Filesystem, MountOptions, default_mount_point, validate_mount_point};
use yukti::udev::{
    UdevEvent, classify_and_extract, classify_device, device_info_from_udev, extract_capabilities,
};

// ── helpers ──────────────────────────────────────────────────────────────

fn make_device_info() -> DeviceInfo {
    let mut info = DeviceInfo::new(
        DeviceId::new("block:sdb1"),
        PathBuf::from("/dev/sdb1"),
        DeviceClass::UsbStorage,
    );
    info.label = Some("MY_USB".into());
    info.model = Some("SanDisk Cruzer".into());
    info.vendor = Some("SanDisk".into());
    info.serial = Some("ABC123".into());
    info.size_bytes = 1024 * 1024 * 1024 * 16; // 16 GB
    info.capabilities = DeviceCapabilities::READ
        | DeviceCapabilities::WRITE
        | DeviceCapabilities::REMOVABLE
        | DeviceCapabilities::HOTPLUG
        | DeviceCapabilities::EJECT;
    info
}

fn make_usb_udev_event() -> UdevEvent {
    let mut props = HashMap::new();
    props.insert("ID_BUS".into(), "usb".into());
    props.insert("ID_VENDOR".into(), "SanDisk".into());
    props.insert("ID_MODEL".into(), "Cruzer_Blade".into());
    props.insert("ID_SERIAL_SHORT".into(), "ABC123".into());
    props.insert("ID_FS_TYPE".into(), "vfat".into());
    props.insert("ID_FS_LABEL".into(), "MYUSB".into());
    props.insert("ID_USB_DRIVER".into(), "usb-storage".into());
    props.insert("ID_PART_ENTRY_SIZE".into(), "31457280".into());

    UdevEvent {
        action: "add".into(),
        sys_path: PathBuf::from("/sys/devices/pci0000:00/usb1/1-1/block/sdb/sdb1"),
        dev_path: Some(PathBuf::from("/dev/sdb1")),
        subsystem: "block".into(),
        dev_type: Some("partition".into()),
        properties: props,
    }
}

fn make_optical_udev_event() -> UdevEvent {
    let mut props = HashMap::new();
    props.insert("ID_CDROM".into(), "1".into());
    props.insert("ID_CDROM_DVD".into(), "1".into());
    props.insert("ID_CDROM_DVD_RW".into(), "1".into());
    props.insert("ID_FS_TYPE".into(), "iso9660".into());
    props.insert("ID_FS_LABEL".into(), "MOVIE_DISC".into());

    UdevEvent {
        action: "change".into(),
        sys_path: PathBuf::from("/sys/devices/pci0000:00/ata1/host0/target0:0:0/0:0:0:0/block/sr0"),
        dev_path: Some(PathBuf::from("/dev/sr0")),
        subsystem: "block".into(),
        dev_type: Some("disk".into()),
        properties: props,
    }
}

fn make_dm_udev_event() -> UdevEvent {
    UdevEvent {
        action: "add".into(),
        sys_path: PathBuf::from("/sys/devices/virtual/block/dm-0"),
        dev_path: Some(PathBuf::from("/dev/dm-0")),
        subsystem: "block".into(),
        dev_type: Some("disk".into()),
        properties: HashMap::new(),
    }
}

fn make_toc() -> DiscToc {
    DiscToc {
        disc_type: DiscType::CdMixed,
        tracks: vec![
            TocEntry {
                number: 1,
                track_type: TrackType::Audio,
                start_lba: 0,
                length_sectors: 22050,
                duration_secs: Some(180.5),
            },
            TocEntry {
                number: 2,
                track_type: TrackType::Audio,
                start_lba: 22050,
                length_sectors: 18000,
                duration_secs: Some(240.0),
            },
            TocEntry {
                number: 3,
                track_type: TrackType::Data,
                start_lba: 40050,
                length_sectors: 100000,
                duration_secs: None,
            },
        ],
        total_size_bytes: 700_000_000,
    }
}

// ── benchmarks ───────────────────────────────────────────────────────────

fn bench_device_id(c: &mut Criterion) {
    let mut group = c.benchmark_group("device_id");

    group.bench_function("create", |b| {
        b.iter(|| DeviceId::new(black_box("block:sdb1")))
    });
    group.bench_function("display", |b| {
        let id = DeviceId::new("block:sdb1");
        b.iter(|| black_box(&id).to_string())
    });
    group.bench_function("from_str", |b| {
        b.iter(|| DeviceId::from(black_box("block:sdb1")))
    });

    group.finish();
}

fn bench_device_info(c: &mut Criterion) {
    let mut group = c.benchmark_group("device_info");

    group.bench_function("create", |b| {
        b.iter(|| {
            DeviceInfo::new(
                DeviceId::new(black_box("block:sdb1")),
                PathBuf::from("/dev/sdb1"),
                DeviceClass::UsbStorage,
            )
        })
    });
    group.bench_function("display_name_label", |b| {
        let info = make_device_info();
        b.iter(|| black_box(&info).display_name())
    });
    group.bench_function("display_name_fallback", |b| {
        let info = DeviceInfo::new(
            DeviceId::new("test"),
            PathBuf::from("/dev/sdc1"),
            DeviceClass::UsbStorage,
        );
        b.iter(|| black_box(&info).display_name())
    });
    group.bench_function("size_display_zero", |b| {
        let info = DeviceInfo::new(
            DeviceId::new("test"),
            PathBuf::from("/dev/sda"),
            DeviceClass::BlockInternal,
        );
        b.iter(|| black_box(&info).size_display())
    });
    group.bench_function("size_display_gb", |b| {
        let info = make_device_info();
        b.iter(|| black_box(&info).size_display())
    });
    group.bench_function("has_capability_hit", |b| {
        let info = make_device_info();
        b.iter(|| black_box(&info).has_capability(DeviceCapability::Eject))
    });
    group.bench_function("has_capability_miss", |b| {
        let info = make_device_info();
        b.iter(|| black_box(&info).has_capability(DeviceCapability::Burn))
    });
    group.bench_function("is_removable", |b| {
        let info = make_device_info();
        b.iter(|| black_box(&info).is_removable())
    });
    group.bench_function("is_mounted", |b| {
        let info = make_device_info();
        b.iter(|| black_box(&info).is_mounted())
    });

    group.finish();
}

fn bench_capabilities(c: &mut Criterion) {
    let mut group = c.benchmark_group("capabilities");

    group.bench_function("from_slice_5", |b| {
        let caps = [
            DeviceCapability::Read,
            DeviceCapability::Write,
            DeviceCapability::Removable,
            DeviceCapability::Hotplug,
            DeviceCapability::Eject,
        ];
        b.iter(|| DeviceCapabilities::from_slice(black_box(&caps)))
    });
    group.bench_function("to_vec_5", |b| {
        let caps = DeviceCapabilities::READ
            | DeviceCapabilities::WRITE
            | DeviceCapabilities::REMOVABLE
            | DeviceCapabilities::HOTPLUG
            | DeviceCapabilities::EJECT;
        b.iter(|| black_box(caps).to_vec())
    });
    group.bench_function("contains_check", |b| {
        let caps =
            DeviceCapabilities::READ | DeviceCapabilities::WRITE | DeviceCapabilities::REMOVABLE;
        b.iter(|| black_box(caps).contains(DeviceCapabilities::WRITE))
    });
    group.bench_function("bitwise_or_all", |b| {
        b.iter(|| {
            black_box(
                DeviceCapabilities::READ
                    | DeviceCapabilities::WRITE
                    | DeviceCapabilities::EJECT
                    | DeviceCapabilities::MEDIA_CHANGE
                    | DeviceCapabilities::REMOVABLE
                    | DeviceCapabilities::TRIM
                    | DeviceCapabilities::HOTPLUG
                    | DeviceCapabilities::TRAY_CONTROL
                    | DeviceCapabilities::AUDIO_PLAYBACK
                    | DeviceCapabilities::BURN,
            )
        })
    });

    group.finish();
}

fn bench_device_event(c: &mut Criterion) {
    let mut group = c.benchmark_group("device_event");

    group.bench_function("create", |b| {
        b.iter(|| {
            DeviceEvent::new(
                DeviceId::new(black_box("block:sdb1")),
                DeviceClass::UsbStorage,
                DeviceEventKind::Attached,
                PathBuf::from("/dev/sdb1"),
            )
        })
    });
    group.bench_function("is_attach", |b| {
        let e = DeviceEvent::new(
            DeviceId::new("block:sdb1"),
            DeviceClass::UsbStorage,
            DeviceEventKind::Attached,
            PathBuf::from("/dev/sdb1"),
        );
        b.iter(|| black_box(&e).is_attach())
    });
    group.bench_function("is_removable", |b| {
        let e = DeviceEvent::new(
            DeviceId::new("block:sdb1"),
            DeviceClass::UsbStorage,
            DeviceEventKind::Attached,
            PathBuf::from("/dev/sdb1"),
        );
        b.iter(|| black_box(&e).is_removable())
    });

    group.finish();
}

fn bench_event_collector(c: &mut Criterion) {
    let mut group = c.benchmark_group("event_collector");

    group.bench_function("push_100", |b| {
        let event = DeviceEvent::new(
            DeviceId::new("block:sdb1"),
            DeviceClass::UsbStorage,
            DeviceEventKind::Attached,
            PathBuf::from("/dev/sdb1"),
        );
        b.iter(|| {
            let collector = EventCollector::new();
            for _ in 0..100 {
                collector.on_event(black_box(&event));
            }
            collector.count()
        })
    });

    group.finish();
}

fn bench_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("serialization");

    let info = make_device_info();
    let json = serde_json::to_string(&info).unwrap();

    group.bench_function("serialize_device_info", |b| {
        b.iter(|| serde_json::to_string(black_box(&info)).unwrap())
    });
    group.bench_function("deserialize_device_info", |b| {
        b.iter(|| serde_json::from_str::<DeviceInfo>(black_box(&json)).unwrap())
    });

    let event = DeviceEvent::new(
        DeviceId::new("block:sdb1"),
        DeviceClass::UsbStorage,
        DeviceEventKind::Attached,
        PathBuf::from("/dev/sdb1"),
    );
    let event_json = serde_json::to_string(&event).unwrap();

    group.bench_function("serialize_event", |b| {
        b.iter(|| serde_json::to_string(black_box(&event)).unwrap())
    });
    group.bench_function("deserialize_event", |b| {
        b.iter(|| serde_json::from_str::<DeviceEvent>(black_box(&event_json)).unwrap())
    });

    group.finish();
}

fn bench_storage(c: &mut Criterion) {
    let mut group = c.benchmark_group("storage");

    group.bench_function("filesystem_parse", |b| {
        b.iter(|| Filesystem::from_str_type(black_box("ext4")))
    });
    group.bench_function("filesystem_parse_case", |b| {
        b.iter(|| Filesystem::from_str_type(black_box("EXT4")))
    });
    group.bench_function("filesystem_parse_unknown", |b| {
        b.iter(|| Filesystem::from_str_type(black_box("zfs")))
    });
    group.bench_function("validate_mount_point_ok", |b| {
        let p = std::path::Path::new("/mnt/usb");
        b.iter(|| validate_mount_point(black_box(p)))
    });
    group.bench_function("validate_mount_point_forbidden", |b| {
        let p = std::path::Path::new("/usr");
        b.iter(|| validate_mount_point(black_box(p)))
    });
    group.bench_function("validate_mount_point_deep", |b| {
        let p = std::path::Path::new("/run/media/user/long-device-name-here");
        b.iter(|| validate_mount_point(black_box(p)))
    });
    group.bench_function("default_mount_point", |b| {
        let info = make_device_info();
        b.iter(|| default_mount_point(black_box(&info)))
    });
    group.bench_function("mount_options_default", |b| b.iter(MountOptions::default));
    group.bench_function("mount_options_builder", |b| {
        b.iter(|| {
            MountOptions::new()
                .mount_point("/mnt/test")
                .read_only(true)
                .fs_type("ext4")
                .option("noexec")
        })
    });
    group.bench_function("filesystem_parse_f2fs", |b| {
        b.iter(|| Filesystem::from_str_type(black_box("f2fs")))
    });
    group.bench_function("filesystem_parse_nfs", |b| {
        b.iter(|| Filesystem::from_str_type(black_box("nfs")))
    });

    group.finish();
}

fn bench_optical(c: &mut Criterion) {
    let mut group = c.benchmark_group("optical");

    group.bench_function("detect_disc_type_cd", |b| {
        b.iter(|| detect_disc_type(black_box("cd"), true, false))
    });
    group.bench_function("detect_disc_type_dvd", |b| {
        b.iter(|| detect_disc_type(black_box("dvd-rom"), false, true))
    });
    group.bench_function("detect_disc_type_bluray", |b| {
        b.iter(|| detect_disc_type(black_box("bd"), false, true))
    });
    group.bench_function("detect_disc_type_unknown", |b| {
        b.iter(|| detect_disc_type(black_box("floppy"), false, false))
    });
    group.bench_function("toc_audio_count", |b| {
        let toc = make_toc();
        b.iter(|| black_box(&toc).audio_track_count())
    });
    group.bench_function("toc_audio_duration", |b| {
        let toc = make_toc();
        b.iter(|| black_box(&toc).total_audio_duration())
    });
    group.bench_function("is_dvd_video", |b| {
        let p = std::path::Path::new("/tmp");
        b.iter(|| is_dvd_video(black_box(p)))
    });

    group.finish();
}

fn bench_device_queries(c: &mut Criterion) {
    let mut group = c.benchmark_group("device_queries");

    group.bench_function("query_permissions", |b| {
        let p = std::path::Path::new("/dev/null");
        b.iter(|| query_permissions(black_box(p)))
    });
    group.bench_function("query_device_health_miss", |b| {
        b.iter(|| query_device_health(black_box("nonexistent_dev")))
    });

    group.finish();
}

fn bench_udev(c: &mut Criterion) {
    let mut group = c.benchmark_group("udev");

    group.bench_function("classify_usb", |b| {
        let event = make_usb_udev_event();
        b.iter(|| classify_device(black_box(&event)))
    });
    group.bench_function("classify_optical", |b| {
        let event = make_optical_udev_event();
        b.iter(|| classify_device(black_box(&event)))
    });
    group.bench_function("classify_dm", |b| {
        let event = make_dm_udev_event();
        b.iter(|| classify_device(black_box(&event)))
    });
    group.bench_function("extract_capabilities_usb", |b| {
        let event = make_usb_udev_event();
        b.iter(|| extract_capabilities(black_box(&event), DeviceClass::UsbStorage))
    });
    group.bench_function("extract_capabilities_optical", |b| {
        let event = make_optical_udev_event();
        b.iter(|| extract_capabilities(black_box(&event), DeviceClass::Optical))
    });
    group.bench_function("classify_and_extract_usb", |b| {
        let event = make_usb_udev_event();
        b.iter(|| classify_and_extract(black_box(&event)))
    });
    group.bench_function("device_info_from_udev", |b| {
        let event = make_usb_udev_event();
        b.iter(|| device_info_from_udev(black_box(&event)).unwrap())
    });
    group.bench_function("device_info_from_udev_optical", |b| {
        let event = make_optical_udev_event();
        b.iter(|| device_info_from_udev(black_box(&event)).unwrap())
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_device_id,
    bench_device_info,
    bench_capabilities,
    bench_device_event,
    bench_event_collector,
    bench_serialization,
    bench_storage,
    bench_optical,
    bench_device_queries,
    bench_udev,
);
criterion_main!(benches);
