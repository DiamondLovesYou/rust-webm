//! A crate for muxing one or more video/audio streams into a `WebM` file.
//!
//! Note that this crate is only for muxing media that has already been encoded with the appropriate codec.
//! Consider a crate such as `vpx` if you need encoding as well.
//!
//! Actual writing of muxed data is done through a [`mux::Writer`], which lets you supply your own implementation.
//! This makes it easy to support muxing to files, in-memory buffers, or whatever else you need. Once you have
//! a [`mux::Writer`], you create a [`mux::SegmentBuilder`] and add the tracks you need. Finally, you create a
//! [`mux::Segment`] with that builder, to which you can add media frames.
//!
//! In typical usage of this library, where you might mux to a `WebM` file, you would do:
//! ```no_run
//! use std::fs::File;
//! use webm::mux::{Error, SegmentBuilder, SegmentMode, VideoCodecId, Writer};
//!
//! fn main() -> Result<(), Error> {
//!     let file = File::create("./my-cool-file.webm").map_err(|_| Error::Unknown)?;
//!     let writer = Writer::new(file);
//!
//!     // Build a segment with a single video track
//!     let builder = SegmentBuilder::new(writer)?;
//!     let builder = builder.set_mode(SegmentMode::Live)?; // Set live mode for streaming
//!     let (builder, video_track) = builder.add_video_track(640, 480, VideoCodecId::VP8, None)?;
//!     let mut segment = builder.build();
//!
//!     // Add some video frames
//!     let encoded_video_frame: &[u8] = &[]; // TODO: Your video data here
//!     let timestamp_ns = 0;
//!     let is_keyframe = true;
//!     segment.add_frame(video_track, encoded_video_frame, timestamp_ns, is_keyframe)?;
//!     // TODO: More video frames
//!
//!     // Done writing frames, finish off the file
//!     segment.finalize(None).map_err(|_| Error::Unknown)?;
//!     Ok(())
//! }
//! ```

use webm_sys as ffi;

pub mod mux {
    mod segment;
    mod writer;

    pub use crate::ffi::mux::TrackNum;
    pub use segment::{Segment, SegmentBuilder};
    pub use writer::Writer;

    use crate::ffi;
    use std::num::NonZeroU64;

    /// This is a copyable handle equivalent to a track number
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct VideoTrack(NonZeroU64);

    impl From<VideoTrack> for TrackNum {
        #[inline]
        fn from(track: VideoTrack) -> Self {
            track.0.get()
        }
    }

    /// This is a copyable handle equivalent to a track number
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct AudioTrack(NonZeroU64);

    impl From<AudioTrack> for TrackNum {
        #[inline]
        fn from(track: AudioTrack) -> Self {
            track.0.get()
        }
    }

    pub trait Track {
        #[must_use]
        fn is_audio(&self) -> bool {
            false
        }

        #[must_use]
        fn is_video(&self) -> bool {
            false
        }

        #[must_use]
        fn track_number(&self) -> TrackNum;
    }

    impl Track for VideoTrack {
        #[inline]
        fn is_video(&self) -> bool {
            true
        }

        #[inline]
        fn track_number(&self) -> TrackNum {
            self.0.get()
        }
    }

    impl Track for AudioTrack {
        #[inline]
        fn is_audio(&self) -> bool {
            true
        }

        #[inline]
        fn track_number(&self) -> TrackNum {
            self.0.get()
        }
    }

    #[derive(Eq, PartialEq, Clone, Copy, Debug)]
    #[repr(u32)]
    pub enum AudioCodecId {
        Opus = ffi::mux::OPUS_CODEC_ID,
        Vorbis = ffi::mux::VORBIS_CODEC_ID,
    }

    impl AudioCodecId {
        const fn get_id(self) -> u32 {
            self as u32
        }
    }

    #[derive(Eq, PartialEq, Clone, Copy, Debug)]
    #[repr(u32)]
    pub enum VideoCodecId {
        VP8 = ffi::mux::VP8_CODEC_ID,
        VP9 = ffi::mux::VP9_CODEC_ID,
        AV1 = ffi::mux::AV1_CODEC_ID,
    }

    impl VideoCodecId {
        const fn get_id(self) -> u32 {
            self as u32
        }
    }

    /// The error type for this entire crate. More specific error types will
    /// be added in the future, hence the current marking as non-exhaustive.
    #[derive(Debug)]
    #[non_exhaustive]
    pub enum Error {
        /// An parameter with an invalid value was passed to a method.
        BadParam,

        /// An unknown error occurred. While this is typically the result of
        /// incorrect parameters to methods, an internal error in libwebm is
        /// also possible.
        Unknown,
    }

    impl Error {
        pub(crate) fn check_code(code: ffi::mux::ResultCode) -> Result<(), Self> {
            match code {
                ffi::mux::ResultCode::Ok => Ok(()),
                ffi::mux::ResultCode::BadParam => Err(Self::BadParam),
                ffi::mux::ResultCode::UnknownLibwebmError => Err(Self::Unknown),
            }
        }
    }

    impl std::fmt::Display for Error {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::BadParam => f.write_str("Bad parameter"),
                Self::Unknown => f.write_str("Unknown error"),
            }
        }
    }

    impl std::error::Error for Error {}

    /// A specification for how pixels in written video frames are subsampled in chroma channels.
    ///
    /// Certain video frame formats (e.g. YUV 4:2:0) have a lower resolution in chroma (Cr/Cb) channels than the
    /// luminance channel. This structure informs video players how that subsampling is done, using a number of
    /// subsampling factors. A factor of zero means no subsampling, and a factor of one means that particular dimension
    /// is half resolution.
    ///
    /// You may use [`ColorSubsampling::default()`] to get a specification of no subsampling in any dimension.
    #[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ColorSubsampling {
        /// The subsampling factor for both chroma channels in the horizontal direction.
        pub chroma_horizontal: u8,

        /// The subsampling factor for both chroma channels in the vertical direction.
        pub chroma_vertical: u8,
    }

    /// A specification of how the range of colors in the input video frames has been clipped.
    ///
    /// Certain screens struggle with the full range of available colors, and video content is thus sometimes tuned to
    /// a restricted range.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub enum ColorRange {
        /// No claim is made as to how colors have been restricted.
        #[default]
        Unspecified = 0,

        /// Color values are restricted to a "broadcast-safe" range.
        Broadcast = 1,

        /// No color clipping is performed.
        Full = 2,
    }

    /// A specification for the segment writing mode.
    ///
    /// This controls how the segment is written and affects features like seeking.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum SegmentMode {
        /// Live mode - optimized for real-time streaming.
        /// In this mode, seeking information may not be available.
        Live,

        /// File mode - optimized for file-based playback.
        /// This enables full seeking and duration information.
        File,
    }

    /// Transfer characteristics (EOTF - Electro-Optical Transfer Function).
    ///
    /// Specifies how the video signal values relate to light output.
    /// See ITU-T H.273 / ISO/IEC 23091-2.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[repr(u64)]
    pub enum TransferCharacteristics {
        /// Rec. ITU-R BT.709
        Bt709 = 1,
        /// Unspecified
        Unspecified = 2,
        /// Rec. ITU-R BT.470-6 System M
        Bt470M = 4,
        /// Rec. ITU-R BT.470-6 System B, G
        Bt470Bg = 5,
        /// Rec. ITU-R BT.601-7 525 or 625
        Bt601 = 6,
        /// SMPTE ST 240
        Smpte240M = 7,
        /// Linear transfer characteristics
        Linear = 8,
        /// Logarithmic transfer (100:1 range)
        Log100 = 9,
        /// Logarithmic transfer (316.22777:1 range)
        Log316 = 10,
        /// IEC 61966-2-4
        Iec61966_2_4 = 11,
        /// Rec. ITU-R BT.1361-0 extended colour gamut system
        Bt1361 = 12,
        /// IEC 61966-2-1 sRGB
        Iec61966_2_1 = 13,
        /// Rec. ITU-R BT.2020-2 (10-bit system)
        Bt2020_10bit = 14,
        /// Rec. ITU-R BT.2020-2 (12-bit system)
        Bt2020_12bit = 15,
        /// SMPTE ST 2084 - Perceptual Quantizer (PQ) for HDR10
        Smpte2084 = 16,
        /// SMPTE ST 428-1
        Smpte428 = 17,
        /// ARIB STD-B67 - Hybrid Log-Gamma (HLG)
        AribStdB67 = 18,
    }

    /// Color primaries specification.
    ///
    /// Defines the chromaticity coordinates of the source primaries.
    /// See ITU-T H.273 / ISO/IEC 23091-2.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[repr(u64)]
    pub enum ColorPrimaries {
        /// Rec. ITU-R BT.709
        Bt709 = 1,
        /// Unspecified
        Unspecified = 2,
        /// Rec. ITU-R BT.470-6 System M
        Bt470M = 4,
        /// Rec. ITU-R BT.470-6 System B, G
        Bt470Bg = 5,
        /// Rec. ITU-R BT.601-7 525 or 625
        Bt601 = 6,
        /// SMPTE ST 240
        Smpte240M = 7,
        /// Generic film (colour filters using Illuminant C)
        Film = 8,
        /// Rec. ITU-R BT.2020 / BT.2100 - Wide color gamut for HDR
        Bt2020 = 9,
        /// SMPTE ST 428-1 (CIE 1931 XYZ)
        Smpte428 = 10,
        /// SMPTE RP 431-2 - DCI P3
        Smpte431 = 11,
        /// SMPTE EG 432-1 - Display P3
        Smpte432 = 12,
        /// EBU Tech. 3213-E
        Ebu3213 = 22,
    }

    /// Matrix coefficients for deriving luma and chroma from RGB.
    ///
    /// See ITU-T H.273 / ISO/IEC 23091-2.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[repr(u64)]
    pub enum MatrixCoefficients {
        /// Identity matrix (RGB)
        Identity = 0,
        /// Rec. ITU-R BT.709
        Bt709 = 1,
        /// Unspecified
        Unspecified = 2,
        /// FCC
        Fcc = 4,
        /// Rec. ITU-R BT.470-6 System B, G
        Bt470Bg = 5,
        /// Rec. ITU-R BT.601-7 525 or 625
        Bt601 = 6,
        /// SMPTE ST 240
        Smpte240M = 7,
        /// `YCgCo`
        YCgCo = 8,
        /// Rec. ITU-R BT.2020-2 non-constant luminance
        Bt2020Ncl = 9,
        /// Rec. ITU-R BT.2020-2 constant luminance
        Bt2020Cl = 10,
        /// SMPTE ST 2085
        Smpte2085 = 11,
        /// Chromaticity-derived non-constant luminance
        ChromaNcl = 12,
        /// Chromaticity-derived constant luminance
        ChromaCl = 13,
        /// `ICtCp` (Rec. ITU-R BT.2100-0)
        ICtCp = 14,
    }

    /// Chromaticity coordinates (CIE 1931 xy).
    ///
    /// Values should be in the range 0.0 to 1.0.
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct Chromaticity {
        pub x: f32,
        pub y: f32,
    }

    impl Chromaticity {
        /// D65 white point (standard for BT.709, BT.2020)
        pub const D65: Self = Self {
            x: 0.3127,
            y: 0.3290,
        };
    }

    /// Display primaries for HDR mastering metadata.
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct DisplayPrimaries {
        pub red: Chromaticity,
        pub green: Chromaticity,
        pub blue: Chromaticity,
    }

    impl DisplayPrimaries {
        /// Rec. ITU-R BT.709 primaries (SDR)
        pub const BT_709: Self = Self {
            red: Chromaticity { x: 0.64, y: 0.33 },
            green: Chromaticity { x: 0.30, y: 0.60 },
            blue: Chromaticity { x: 0.15, y: 0.06 },
        };

        /// Rec. ITU-R BT.2020 primaries (HDR/WCG)
        pub const BT_2020: Self = Self {
            red: Chromaticity { x: 0.708, y: 0.292 },
            green: Chromaticity { x: 0.170, y: 0.797 },
            blue: Chromaticity { x: 0.131, y: 0.046 },
        };

        /// DCI-P3 primaries
        pub const DCI_P3: Self = Self {
            red: Chromaticity { x: 0.680, y: 0.320 },
            green: Chromaticity { x: 0.265, y: 0.690 },
            blue: Chromaticity { x: 0.150, y: 0.060 },
        };
    }

    /// SMPTE ST 2086 mastering display metadata.
    ///
    /// Specifies the color volume and luminance range of the display used for mastering HDR content.
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct MasteringDisplayMetadata {
        /// Maximum luminance in candelas per square meter (cd/m² or nits).
        /// Typical values: 1000.0, 4000.0, 10000.0
        pub luminance_max: f32,

        /// Minimum luminance in candelas per square meter (cd/m² or nits).
        /// Typical values: 0.0001, 0.001, 0.01, 0.05
        pub luminance_min: f32,

        /// Display primaries (red, green, blue chromaticity coordinates)
        pub primaries: DisplayPrimaries,

        /// White point chromaticity coordinates
        pub white_point: Chromaticity,
    }

    /// HDR10 static metadata.
    ///
    /// Includes both content light level metadata and mastering display metadata.
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct HdrMetadata {
        /// Maximum Content Light Level in cd/m² (nits).
        /// The maximum light level of any single pixel in the entire video.
        /// Typical range: 1000-4000 nits.
        pub max_cll: u64,

        /// Maximum Frame-Average Light Level in cd/m² (nits).
        /// The maximum average light level of any single frame in the video.
        /// Typical range: 100-1000 nits.
        pub max_fall: u64,

        /// Optional SMPTE ST 2086 mastering metadata.
        pub mastering_metadata: Option<MasteringDisplayMetadata>,
    }
}
