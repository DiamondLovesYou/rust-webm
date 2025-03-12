//! A crate for muxing one or more video/audio streams into a WebM file.
//!
//! Note that this crate is only for muxing media that has already been encoded with the appropriate codec.
//! Consider a crate such as `vpx` if you need encoding as well.
//!
//! Actual writing of muxed data is done through a [`mux::Writer`], which lets you supply your own implementation.
//! This makes it easy to support muxing to files, in-memory buffers, or whatever else you need. Once you have
//! a [`mux::Writer`], you create a [`mux::SegmentBuilder`] and add the tracks you need. Finally, you create a
//! [`mux::Segment`] with that builder, to which you can add media frames.
//!
//! In typical usage of this library, where you might mux to a WebM file, you would do:
//! ```no_run
//! use std::fs::File;
//! use webm::mux::{SegmentBuilder, VideoCodecId, Writer};
//!
//! let file = File::open("./my-cool-file.webm").unwrap();
//! let writer = Writer::new(file);
//!
//! // Build a segment with a single video track
//! let builder = SegmentBuilder::new(writer).unwrap();
//! let (builder, video_track) = builder.add_video_track(640, 480, VideoCodecId::VP8, None).unwrap();
//! let mut segment = builder.build();
//!
//! // Add some video frames
//! let encoded_video_frame: &[u8] = &[]; // TODO: Your video data here
//! let timestamp_ns = 0;
//! let is_keyframe = true;
//! segment.add_frame(video_track, encoded_video_frame, timestamp_ns, is_keyframe).unwrap();
//! // TODO: More video frames
//!
//! // Done writing frames, finish off the file
//! _ = segment.finalize(None).inspect_err(|_| eprintln!("Could not finalize WebM file"));
//! ```

use webm_sys as ffi;

pub mod mux {
    mod segment;
    mod writer;

    pub use {
        crate::ffi::mux::TrackNum,
        segment::{Segment, SegmentBuilder},
        writer::Writer,
    };

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
}
