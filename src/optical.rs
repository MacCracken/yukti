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
        self.tracks
            .iter()
            .filter_map(|t| t.duration_secs)
            .sum()
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
    } else if media_type.eq_ignore_ascii_case("dvd") || media_type.eq_ignore_ascii_case("dvd-rom")
    {
        if has_data {
            DiscType::DvdRom
        } else {
            DiscType::DvdRom // DVD without data is still classified as DvdRom
        }
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
    } else if media_type.eq_ignore_ascii_case("blank")
        || media_type.eq_ignore_ascii_case("no_disc")
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
        assert_eq!(detect_disc_type("dvd-r", false, true), DiscType::DvdWritable);
        assert_eq!(detect_disc_type("dvd-rw", false, true), DiscType::DvdWritable);
        assert_eq!(detect_disc_type("dvd+r", false, true), DiscType::DvdWritable);
        assert_eq!(detect_disc_type("dvd+rw", false, true), DiscType::DvdWritable);
    }

    #[test]
    fn test_detect_dvd_video() {
        assert_eq!(detect_disc_type("dvd-video", false, true), DiscType::DvdVideo);
    }

    #[test]
    fn test_detect_bluray() {
        assert_eq!(detect_disc_type("bd", false, true), DiscType::BluRay);
        assert_eq!(detect_disc_type("blu-ray", false, true), DiscType::BluRay);
        assert_eq!(detect_disc_type("bd-rom", false, true), DiscType::BluRay);
    }

    #[test]
    fn test_detect_bluray_video() {
        assert_eq!(detect_disc_type("bd-video", false, true), DiscType::BluRayVideo);
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
}
