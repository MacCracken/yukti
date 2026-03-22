//! Optical drive operations — disc type detection, tray control, TOC reading.

use serde::{Deserialize, Serialize};

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
        match self {
            Self::CdAudio => write!(f, "CD-DA (Audio)"),
            Self::CdData => write!(f, "CD-ROM (Data)"),
            Self::CdMixed => write!(f, "CD (Mixed Mode)"),
            Self::DvdRom => write!(f, "DVD-ROM"),
            Self::DvdWritable => write!(f, "DVD-R/RW"),
            Self::DvdVideo => write!(f, "DVD Video"),
            Self::BluRay => write!(f, "Blu-ray"),
            Self::BluRayVideo => write!(f, "Blu-ray Video"),
            Self::Blank => write!(f, "Blank"),
            Self::Unknown => write!(f, "Unknown"),
        }
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
        self.tracks
            .iter()
            .filter_map(|t| t.duration_secs)
            .sum()
    }
}

/// Detect disc type from device capabilities and media info.
///
/// In production, this reads from sysfs:
/// - `/sys/block/sr0/device/media` or `cdrom_id` udev helper
/// - SCSI GET_CONFIGURATION / READ_DISC_INFORMATION commands
///
/// This function provides the detection logic given already-read properties.
pub fn detect_disc_type(media_type: &str, has_audio: bool, has_data: bool) -> DiscType {
    match media_type.to_lowercase().as_str() {
        "cd" | "cd-rom" if has_audio && has_data => DiscType::CdMixed,
        "cd" | "cd-rom" if has_audio => DiscType::CdAudio,
        "cd" | "cd-rom" => DiscType::CdData,
        "dvd" | "dvd-rom" if has_data => DiscType::DvdRom,
        "dvd-r" | "dvd-rw" | "dvd+r" | "dvd+rw" => DiscType::DvdWritable,
        "dvd-video" => DiscType::DvdVideo,
        "bd" | "blu-ray" | "bd-rom" => DiscType::BluRay,
        "bd-video" => DiscType::BluRayVideo,
        "blank" | "no_disc" => DiscType::Blank,
        _ => DiscType::Unknown,
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
    }

    #[test]
    fn test_disc_type_data() {
        assert!(DiscType::CdData.has_data());
        assert!(DiscType::DvdRom.has_data());
        assert!(DiscType::BluRay.has_data());
        assert!(!DiscType::CdAudio.has_data());
        assert!(!DiscType::Blank.has_data());
    }

    #[test]
    fn test_disc_type_writable() {
        assert!(DiscType::DvdWritable.is_writable());
        assert!(DiscType::Blank.is_writable());
        assert!(!DiscType::CdData.is_writable());
    }

    #[test]
    fn test_detect_cd_audio() {
        assert_eq!(detect_disc_type("cd", true, false), DiscType::CdAudio);
    }

    #[test]
    fn test_detect_cd_data() {
        assert_eq!(detect_disc_type("cd-rom", false, true), DiscType::CdData);
    }

    #[test]
    fn test_detect_cd_mixed() {
        assert_eq!(detect_disc_type("cd", true, true), DiscType::CdMixed);
    }

    #[test]
    fn test_detect_dvd() {
        assert_eq!(detect_disc_type("dvd-rom", false, true), DiscType::DvdRom);
        assert_eq!(detect_disc_type("dvd-r", false, true), DiscType::DvdWritable);
    }

    #[test]
    fn test_detect_bluray() {
        assert_eq!(detect_disc_type("bd", false, true), DiscType::BluRay);
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
    fn test_disc_type_display() {
        assert_eq!(DiscType::CdAudio.to_string(), "CD-DA (Audio)");
        assert_eq!(DiscType::DvdRom.to_string(), "DVD-ROM");
        assert_eq!(DiscType::BluRay.to_string(), "Blu-ray");
    }
}
