//! Optical drive operations — disc type detection, tray control, TOC reading.

use serde::{Deserialize, Serialize};
use std::path::Path;

#[cfg(target_os = "linux")]
use tracing::{debug, error, info};

use crate::error::{Result, YantraError};

// ---------------------------------------------------------------------------
// Linux ioctl constants
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
mod ioctl {
    pub const CDROMEJECT: libc::c_ulong = 0x5309;
    pub const CDROMCLOSETRAY: libc::c_ulong = 0x5319;
    pub const CDROM_DRIVE_STATUS: libc::c_ulong = 0x5326;
    pub const CDROMREADTOCHDR: libc::c_ulong = 0x5305;
    pub const CDROMREADTOCENTRY: libc::c_ulong = 0x5306;
    pub const CDROM_LBA: u8 = 0x01;
    pub const CDROM_LEADOUT: u8 = 0xAA;

    // Drive status return values
    pub const CDS_NO_DISC: i32 = 1;
    pub const CDS_TRAY_OPEN: i32 = 2;
    pub const CDS_DRIVE_NOT_READY: i32 = 3;
    pub const CDS_DISC_OK: i32 = 4;
}

// ---------------------------------------------------------------------------
// Linux kernel FFI structs
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
mod ffi {
    #[repr(C)]
    pub struct CdromTocHdr {
        pub first_track: u8,
        pub last_track: u8,
    }

    #[repr(C)]
    pub struct CdromTocEntry {
        pub track: u8,
        pub adr_ctrl: u8,
        pub format: u8,
        pub addr: CdromAddr,
        pub datamode: u8,
    }

    #[repr(C)]
    pub union CdromAddr {
        pub lba: i32,
        pub msf: CdromMsf0,
    }

    #[repr(C)]
    #[derive(Copy, Clone)]
    pub struct CdromMsf0 {
        pub minute: u8,
        pub second: u8,
        pub frame: u8,
    }
}

// ---------------------------------------------------------------------------
// Linux helper: open optical device
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
fn open_optical_device(dev_path: &Path) -> Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;

    std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(dev_path)
        .map_err(|e| {
            let raw = e.raw_os_error().unwrap_or(0);
            match raw {
                libc::EACCES | libc::EPERM => YantraError::PermissionDenied {
                    operation: "open".into(),
                    path: dev_path.to_path_buf(),
                },
                123 /* ENOMEDIUM */ => YantraError::NoMedia {
                    device: dev_path.display().to_string(),
                },
                _ => YantraError::Io(e),
            }
        })
}

// ---------------------------------------------------------------------------
// Linux helper: map ioctl errno to YantraError
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
fn map_ioctl_error(dev_path: &Path, op: &str) -> YantraError {
    let err = std::io::Error::last_os_error();
    let raw = err.raw_os_error().unwrap_or(0);
    match raw {
        libc::EACCES | libc::EPERM => YantraError::PermissionDenied {
            operation: op.into(),
            path: dev_path.to_path_buf(),
        },
        123 /* ENOMEDIUM */ => YantraError::NoMedia {
            device: dev_path.display().to_string(),
        },
        _ => YantraError::TrayFailed {
            reason: format!("{op}: {err}"),
        },
    }
}

// ---------------------------------------------------------------------------
// Public Linux functions
// ---------------------------------------------------------------------------

/// Open the optical drive tray (eject).
#[cfg(target_os = "linux")]
pub fn open_tray(dev_path: &Path) -> Result<()> {
    use std::os::unix::io::AsRawFd;

    info!(device = %dev_path.display(), "opening tray");
    let file = open_optical_device(dev_path)?;
    let fd = file.as_raw_fd();
    let ret = unsafe { libc::ioctl(fd, ioctl::CDROMEJECT as libc::c_ulong) };
    if ret < 0 {
        error!(device = %dev_path.display(), "open tray ioctl failed");
        return Err(map_ioctl_error(dev_path, "eject"));
    }
    info!(device = %dev_path.display(), "tray opened");
    Ok(())
}

/// Close the optical drive tray.
#[cfg(target_os = "linux")]
pub fn close_tray(dev_path: &Path) -> Result<()> {
    use std::os::unix::io::AsRawFd;

    info!(device = %dev_path.display(), "closing tray");
    let file = open_optical_device(dev_path)?;
    let fd = file.as_raw_fd();
    let ret = unsafe { libc::ioctl(fd, ioctl::CDROMCLOSETRAY as libc::c_ulong) };
    if ret < 0 {
        error!(device = %dev_path.display(), "close tray ioctl failed");
        return Err(map_ioctl_error(dev_path, "close tray"));
    }
    info!(device = %dev_path.display(), "tray closed");
    Ok(())
}

/// Query the drive/tray status.
#[cfg(target_os = "linux")]
pub fn drive_status(dev_path: &Path) -> Result<TrayState> {
    use std::os::unix::io::AsRawFd;

    debug!(device = %dev_path.display(), "querying drive status");
    let file = open_optical_device(dev_path)?;
    let fd = file.as_raw_fd();
    let ret = unsafe { libc::ioctl(fd, ioctl::CDROM_DRIVE_STATUS as libc::c_ulong) };
    if ret < 0 {
        error!(device = %dev_path.display(), "drive status ioctl failed");
        return Err(map_ioctl_error(dev_path, "drive status"));
    }
    match ret {
        ioctl::CDS_TRAY_OPEN => Ok(TrayState::Open),
        ioctl::CDS_DISC_OK | ioctl::CDS_NO_DISC | ioctl::CDS_DRIVE_NOT_READY => {
            Ok(TrayState::Closed)
        }
        _ => Ok(TrayState::Unknown),
    }
}

/// Read the table of contents from a CD in the given drive.
///
/// Reads the TOC header, then each track entry plus the lead-out, and
/// computes track lengths and durations (75 frames/sec for audio).
#[cfg(target_os = "linux")]
pub fn read_toc(dev_path: &Path) -> Result<DiscToc> {
    use std::os::unix::io::AsRawFd;

    info!(device = %dev_path.display(), "reading disc TOC");
    let file = open_optical_device(dev_path)?;
    let fd = file.as_raw_fd();

    // Read TOC header to get first/last track numbers.
    let mut hdr = ffi::CdromTocHdr {
        first_track: 0,
        last_track: 0,
    };
    let ret = unsafe {
        libc::ioctl(
            fd,
            ioctl::CDROMREADTOCHDR as libc::c_ulong,
            &mut hdr as *mut ffi::CdromTocHdr,
        )
    };
    if ret < 0 {
        return Err(map_ioctl_error(dev_path, "read TOC header"));
    }

    let first = hdr.first_track;
    let last = hdr.last_track;

    // Read LBA for each track plus the lead-out (0xAA).
    let track_numbers: Vec<u8> = (first..=last)
        .chain(std::iter::once(ioctl::CDROM_LEADOUT))
        .collect();

    let mut lba_values: Vec<(u8, i32, bool)> = Vec::with_capacity(track_numbers.len());

    for &track_num in &track_numbers {
        let mut entry = ffi::CdromTocEntry {
            track: track_num,
            adr_ctrl: 0,
            format: ioctl::CDROM_LBA,
            addr: ffi::CdromAddr { lba: 0 },
            datamode: 0,
        };
        let ret = unsafe {
            libc::ioctl(
                fd,
                ioctl::CDROMREADTOCENTRY as libc::c_ulong,
                &mut entry as *mut ffi::CdromTocEntry,
            )
        };
        if ret < 0 {
            return Err(map_ioctl_error(dev_path, "read TOC entry"));
        }
        let lba = unsafe { entry.addr.lba };
        // Bit 2 of adr_ctrl indicates data track (control field bit 2).
        let is_data = (entry.adr_ctrl & 0x04) != 0;
        lba_values.push((track_num, lba, is_data));
    }

    // Build track entries by computing length from consecutive LBA values.
    let mut tracks = Vec::with_capacity((last - first + 1) as usize);
    let mut has_audio = false;
    let mut has_data = false;

    for i in 0..(lba_values.len() - 1) {
        let (track_num, start_lba, is_data_track) = lba_values[i];
        let next_lba = lba_values[i + 1].1;
        let length_sectors = (next_lba - start_lba).max(0) as u64;

        let track_type = if is_data_track {
            has_data = true;
            TrackType::Data
        } else {
            has_audio = true;
            TrackType::Audio
        };

        let duration_secs = if track_type == TrackType::Audio {
            Some(length_sectors as f64 / 75.0)
        } else {
            None
        };

        tracks.push(TocEntry {
            number: track_num as u32,
            track_type,
            start_lba: start_lba.max(0) as u64,
            length_sectors,
            duration_secs,
        });
    }

    // Determine total size: lead-out LBA * 2048 bytes/sector.
    let leadout_lba = lba_values.last().map(|v| v.1).unwrap_or(0);
    let total_size_bytes = leadout_lba.max(0) as u64 * 2048;

    // Determine disc type based on track contents.
    let disc_type = classify_toc_tracks(has_audio, has_data);

    info!(
        device = %dev_path.display(),
        disc_type = %disc_type,
        tracks = tracks.len(),
        total_size_bytes,
        "TOC read complete"
    );

    Ok(DiscToc {
        disc_type,
        tracks,
        total_size_bytes,
    })
}

/// Classify disc type from TOC track information.
fn classify_toc_tracks(has_audio: bool, has_data: bool) -> DiscType {
    match (has_audio, has_data) {
        (true, true) => DiscType::CdMixed,
        (true, false) => DiscType::CdAudio,
        (false, true) => DiscType::CdData,
        (false, false) => DiscType::Unknown,
    }
}

/// Type of optical media.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DiscType {
    /// Audio CD (CDDA).
    CdAudio,
    /// Data CD (ISO 9660).
    CdData,
    /// Mixed mode CD (audio + data).
    CdMixed,
    /// DVD-ROM (read-only data).
    DvdRom,
    /// DVD-R/RW (writable).
    DvdWritable,
    /// DVD Video.
    DvdVideo,
    /// Blu-ray disc.
    BluRay,
    /// Blu-ray video.
    BluRayVideo,
    /// Blank/unformatted disc.
    Blank,
    /// Unknown disc type.
    Unknown,
}

impl DiscType {
    /// Whether this disc type contains audio tracks.
    pub fn has_audio(&self) -> bool {
        matches!(self, Self::CdAudio | Self::CdMixed)
    }

    /// Whether this disc type contains data.
    pub fn has_data(&self) -> bool {
        matches!(
            self,
            Self::CdData
                | Self::CdMixed
                | Self::DvdRom
                | Self::DvdWritable
                | Self::DvdVideo
                | Self::BluRay
                | Self::BluRayVideo
        )
    }

    /// Whether this disc type is writable.
    pub fn is_writable(&self) -> bool {
        matches!(self, Self::DvdWritable | Self::Blank)
    }
}

impl std::fmt::Display for DiscType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::CdAudio => "CD-DA (Audio)",
            Self::CdData => "CD-ROM (Data)",
            Self::CdMixed => "CD (Mixed Mode)",
            Self::DvdRom => "DVD-ROM",
            Self::DvdWritable => "DVD-R/RW",
            Self::DvdVideo => "DVD Video",
            Self::BluRay => "Blu-ray",
            Self::BluRayVideo => "Blu-ray Video",
            Self::Blank => "Blank",
            Self::Unknown => "Unknown",
        };
        f.write_str(s)
    }
}

/// Tray state of an optical drive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrayState {
    Open,
    Closed,
    Unknown,
}

/// A track entry in a disc's table of contents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TocEntry {
    /// Track number (1-based).
    pub number: u32,
    /// Track type.
    pub track_type: TrackType,
    /// Start sector (LBA).
    pub start_lba: u64,
    /// Length in sectors.
    pub length_sectors: u64,
    /// Duration in seconds (for audio tracks).
    pub duration_secs: Option<f64>,
}

/// Type of track on a disc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrackType {
    Audio,
    Data,
    Unknown,
}

/// Table of contents for a disc.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscToc {
    pub disc_type: DiscType,
    pub tracks: Vec<TocEntry>,
    pub total_size_bytes: u64,
}

impl DiscToc {
    /// Number of audio tracks.
    pub fn audio_track_count(&self) -> usize {
        self.tracks
            .iter()
            .filter(|t| t.track_type == TrackType::Audio)
            .count()
    }

    /// Number of data tracks.
    pub fn data_track_count(&self) -> usize {
        self.tracks
            .iter()
            .filter(|t| t.track_type == TrackType::Data)
            .count()
    }

    /// Total duration of audio tracks in seconds.
    pub fn total_audio_duration(&self) -> f64 {
        self.tracks.iter().filter_map(|t| t.duration_secs).sum()
    }
}

/// Detect disc type from device capabilities and media info.
/// Zero-allocation: uses case-insensitive ASCII comparison.
pub fn detect_disc_type(media_type: &str, has_audio: bool, has_data: bool) -> DiscType {
    if media_type.eq_ignore_ascii_case("cd") || media_type.eq_ignore_ascii_case("cd-rom") {
        if has_audio && has_data {
            DiscType::CdMixed
        } else if has_audio {
            DiscType::CdAudio
        } else {
            DiscType::CdData
        }
    } else if media_type.eq_ignore_ascii_case("dvd") || media_type.eq_ignore_ascii_case("dvd-rom") {
        DiscType::DvdRom
    } else if media_type.eq_ignore_ascii_case("dvd-r")
        || media_type.eq_ignore_ascii_case("dvd-rw")
        || media_type.eq_ignore_ascii_case("dvd+r")
        || media_type.eq_ignore_ascii_case("dvd+rw")
    {
        DiscType::DvdWritable
    } else if media_type.eq_ignore_ascii_case("dvd-video") {
        DiscType::DvdVideo
    } else if media_type.eq_ignore_ascii_case("bd")
        || media_type.eq_ignore_ascii_case("blu-ray")
        || media_type.eq_ignore_ascii_case("bd-rom")
    {
        DiscType::BluRay
    } else if media_type.eq_ignore_ascii_case("bd-video") {
        DiscType::BluRayVideo
    } else if media_type.eq_ignore_ascii_case("blank") || media_type.eq_ignore_ascii_case("no_disc")
    {
        DiscType::Blank
    } else {
        DiscType::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disc_type_audio() {
        assert!(DiscType::CdAudio.has_audio());
        assert!(DiscType::CdMixed.has_audio());
        assert!(!DiscType::DvdRom.has_audio());
        assert!(!DiscType::CdData.has_audio());
        assert!(!DiscType::BluRay.has_audio());
    }

    #[test]
    fn test_disc_type_data() {
        assert!(DiscType::CdData.has_data());
        assert!(DiscType::CdMixed.has_data());
        assert!(DiscType::DvdRom.has_data());
        assert!(DiscType::DvdWritable.has_data());
        assert!(DiscType::DvdVideo.has_data());
        assert!(DiscType::BluRay.has_data());
        assert!(DiscType::BluRayVideo.has_data());
        assert!(!DiscType::CdAudio.has_data());
        assert!(!DiscType::Blank.has_data());
        assert!(!DiscType::Unknown.has_data());
    }

    #[test]
    fn test_disc_type_writable() {
        assert!(DiscType::DvdWritable.is_writable());
        assert!(DiscType::Blank.is_writable());
        assert!(!DiscType::CdData.is_writable());
        assert!(!DiscType::DvdRom.is_writable());
        assert!(!DiscType::BluRay.is_writable());
    }

    #[test]
    fn test_disc_type_display_all() {
        assert_eq!(DiscType::CdAudio.to_string(), "CD-DA (Audio)");
        assert_eq!(DiscType::CdData.to_string(), "CD-ROM (Data)");
        assert_eq!(DiscType::CdMixed.to_string(), "CD (Mixed Mode)");
        assert_eq!(DiscType::DvdRom.to_string(), "DVD-ROM");
        assert_eq!(DiscType::DvdWritable.to_string(), "DVD-R/RW");
        assert_eq!(DiscType::DvdVideo.to_string(), "DVD Video");
        assert_eq!(DiscType::BluRay.to_string(), "Blu-ray");
        assert_eq!(DiscType::BluRayVideo.to_string(), "Blu-ray Video");
        assert_eq!(DiscType::Blank.to_string(), "Blank");
        assert_eq!(DiscType::Unknown.to_string(), "Unknown");
    }

    #[test]
    fn test_detect_cd_audio() {
        assert_eq!(detect_disc_type("cd", true, false), DiscType::CdAudio);
        assert_eq!(detect_disc_type("CD-ROM", true, false), DiscType::CdAudio);
    }

    #[test]
    fn test_detect_cd_data() {
        assert_eq!(detect_disc_type("cd-rom", false, true), DiscType::CdData);
        assert_eq!(detect_disc_type("cd-rom", false, false), DiscType::CdData);
    }

    #[test]
    fn test_detect_cd_mixed() {
        assert_eq!(detect_disc_type("cd", true, true), DiscType::CdMixed);
    }

    #[test]
    fn test_detect_dvd() {
        assert_eq!(detect_disc_type("dvd-rom", false, true), DiscType::DvdRom);
        assert_eq!(detect_disc_type("dvd", false, true), DiscType::DvdRom);
        assert_eq!(
            detect_disc_type("dvd-r", false, true),
            DiscType::DvdWritable
        );
        assert_eq!(
            detect_disc_type("dvd-rw", false, true),
            DiscType::DvdWritable
        );
        assert_eq!(
            detect_disc_type("dvd+r", false, true),
            DiscType::DvdWritable
        );
        assert_eq!(
            detect_disc_type("dvd+rw", false, true),
            DiscType::DvdWritable
        );
    }

    #[test]
    fn test_detect_dvd_video() {
        assert_eq!(
            detect_disc_type("dvd-video", false, true),
            DiscType::DvdVideo
        );
    }

    #[test]
    fn test_detect_bluray() {
        assert_eq!(detect_disc_type("bd", false, true), DiscType::BluRay);
        assert_eq!(detect_disc_type("blu-ray", false, true), DiscType::BluRay);
        assert_eq!(detect_disc_type("bd-rom", false, true), DiscType::BluRay);
    }

    #[test]
    fn test_detect_bluray_video() {
        assert_eq!(
            detect_disc_type("bd-video", false, true),
            DiscType::BluRayVideo
        );
    }

    #[test]
    fn test_detect_blank() {
        assert_eq!(detect_disc_type("blank", false, false), DiscType::Blank);
        assert_eq!(detect_disc_type("no_disc", false, false), DiscType::Blank);
    }

    #[test]
    fn test_detect_unknown() {
        assert_eq!(detect_disc_type("floppy", false, false), DiscType::Unknown);
        assert_eq!(detect_disc_type("", false, false), DiscType::Unknown);
    }

    #[test]
    fn test_detect_case_insensitive() {
        assert_eq!(detect_disc_type("CD", true, false), DiscType::CdAudio);
        assert_eq!(detect_disc_type("DVD-ROM", false, true), DiscType::DvdRom);
        assert_eq!(detect_disc_type("BD", false, true), DiscType::BluRay);
        assert_eq!(detect_disc_type("BLANK", false, false), DiscType::Blank);
    }

    #[test]
    fn test_toc() {
        let toc = DiscToc {
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
        };
        assert_eq!(toc.audio_track_count(), 2);
        assert_eq!(toc.data_track_count(), 1);
        assert!((toc.total_audio_duration() - 420.5).abs() < 0.01);
    }

    #[test]
    fn test_toc_empty() {
        let toc = DiscToc {
            disc_type: DiscType::Blank,
            tracks: vec![],
            total_size_bytes: 0,
        };
        assert_eq!(toc.audio_track_count(), 0);
        assert_eq!(toc.data_track_count(), 0);
        assert_eq!(toc.total_audio_duration(), 0.0);
    }

    #[test]
    fn test_tray_state_serde() {
        let state = TrayState::Open;
        let json = serde_json::to_string(&state).unwrap();
        let roundtrip: TrayState = serde_json::from_str(&json).unwrap();
        assert_eq!(state, roundtrip);
    }

    #[test]
    fn test_track_type_serde() {
        let audio = TrackType::Audio;
        let json = serde_json::to_string(&audio).unwrap();
        let roundtrip: TrackType = serde_json::from_str(&json).unwrap();
        assert_eq!(audio, roundtrip);
    }

    #[test]
    fn test_toc_serde() {
        let toc = DiscToc {
            disc_type: DiscType::CdAudio,
            tracks: vec![TocEntry {
                number: 1,
                track_type: TrackType::Audio,
                start_lba: 0,
                length_sectors: 22050,
                duration_secs: Some(180.5),
            }],
            total_size_bytes: 100_000,
        };
        let json = serde_json::to_string(&toc).unwrap();
        let roundtrip: DiscToc = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.disc_type, DiscType::CdAudio);
        assert_eq!(roundtrip.tracks.len(), 1);
    }

    // -----------------------------------------------------------------------
    // Tests for classify_toc_tracks (pure logic, no hardware)
    // -----------------------------------------------------------------------

    #[test]
    fn test_classify_audio_only() {
        assert_eq!(classify_toc_tracks(true, false), DiscType::CdAudio);
    }

    #[test]
    fn test_classify_data_only() {
        assert_eq!(classify_toc_tracks(false, true), DiscType::CdData);
    }

    #[test]
    fn test_classify_mixed() {
        assert_eq!(classify_toc_tracks(true, true), DiscType::CdMixed);
    }

    #[test]
    fn test_classify_empty() {
        assert_eq!(classify_toc_tracks(false, false), DiscType::Unknown);
    }

    // -----------------------------------------------------------------------
    // Linux ioctl constant sanity checks
    // -----------------------------------------------------------------------

    #[cfg(target_os = "linux")]
    #[test]
    fn test_ioctl_constants() {
        assert_eq!(ioctl::CDROMEJECT, 0x5309);
        assert_eq!(ioctl::CDROMCLOSETRAY, 0x5319);
        assert_eq!(ioctl::CDROM_DRIVE_STATUS, 0x5326);
        assert_eq!(ioctl::CDROMREADTOCHDR, 0x5305);
        assert_eq!(ioctl::CDROMREADTOCENTRY, 0x5306);
        assert_eq!(ioctl::CDROM_LBA, 0x01);
        assert_eq!(ioctl::CDROM_LEADOUT, 0xAA);
        assert_eq!(ioctl::CDS_NO_DISC, 1);
        assert_eq!(ioctl::CDS_TRAY_OPEN, 2);
        assert_eq!(ioctl::CDS_DRIVE_NOT_READY, 3);
        assert_eq!(ioctl::CDS_DISC_OK, 4);
    }

    // -----------------------------------------------------------------------
    // Hardware tests (require an actual optical drive) — always #[ignore]
    // -----------------------------------------------------------------------

    #[cfg(target_os = "linux")]
    #[test]
    #[ignore]
    fn test_open_tray_hardware() {
        let dev = std::path::Path::new("/dev/sr0");
        // This will fail without a real drive; that is expected.
        let _ = open_tray(dev);
    }

    #[cfg(target_os = "linux")]
    #[test]
    #[ignore]
    fn test_close_tray_hardware() {
        let dev = std::path::Path::new("/dev/sr0");
        let _ = close_tray(dev);
    }

    #[cfg(target_os = "linux")]
    #[test]
    #[ignore]
    fn test_drive_status_hardware() {
        let dev = std::path::Path::new("/dev/sr0");
        let _ = drive_status(dev);
    }

    #[cfg(target_os = "linux")]
    #[test]
    #[ignore]
    fn test_read_toc_hardware() {
        let dev = std::path::Path::new("/dev/sr0");
        let _ = read_toc(dev);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_open_device_nonexistent() {
        let dev = std::path::Path::new("/dev/sr_nonexistent_yantra_test");
        let result = open_optical_device(dev);
        assert!(result.is_err());
    }
}
