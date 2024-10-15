//! A crate for muxing one or more video/audio streams into a WebM file.
//!
//! Note that this crate is only for muxing media that has already been encoded with the appropriate codec.
//! Consider a crate such as `vpx` if you need encoding as well.
//!
//! Actual writing of muxed data is done through a [`mux::Writer`], which lets you supply your own implementation.
//! This makes it easy to support muxing to files, in-memory buffers, or whatever else you need. Once you have
//! a [`mux::Writer`], you can create a [`mux::Segment`] to which you can add tracks and media frames.
//!
//! In typical usage of this library, where you might mux to a WebM file, you would do:
//! ```no_run
//! use std::fs::File;
//! use webm::mux::{Segment, VideoCodecId, Writer};
//!
//! let file = File::open("./my-cool-file.webm").unwrap();
//! let writer = Writer::new(file);
//! let mut segment = Segment::new(writer).unwrap();
//!
//! // Add some video data
//! let video_track = segment.add_video_track(640, 480, None, VideoCodecId::VP8).unwrap();
//! let encoded_video_frame: &[u8] = &[]; // TODO: Your video data here
//! segment.add_frame(video_track, encoded_video_frame, 0, true).unwrap();
//! // TODO: More video frames
//!
//! // Done writing frames, finish off the file
//! _ = segment.finalize(None).inspect_err(|_| eprintln!("Could not finalize WebM file"));
//! ```

extern crate webm_sys as ffi;

pub mod mux {
    mod segment;
    mod writer;

    pub use {ffi::mux::TrackNum, segment::Segment, writer::Writer};

    use crate::ffi;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct VideoTrack(TrackNum);

    impl From<VideoTrack> for TrackNum {
        fn from(track: VideoTrack) -> Self {
            track.0
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct AudioTrack(TrackNum);

    impl From<AudioTrack> for TrackNum {
        fn from(track: AudioTrack) -> Self {
            track.0
        }
    }

    pub trait Track {
        fn is_audio(&self) -> bool {
            false
        }
        fn is_video(&self) -> bool {
            false
        }

        #[must_use]
        fn track_number(&self) -> TrackNum;
    }

    impl Track for VideoTrack {
        fn is_video(&self) -> bool {
            true
        }

        #[must_use]
        fn track_number(&self) -> TrackNum {
            self.0
        }
    }

    impl Track for AudioTrack {
        fn is_audio(&self) -> bool {
            true
        }

        #[must_use]
        fn track_number(&self) -> TrackNum {
            self.0
        }
    }

    #[derive(Eq, PartialEq, Clone, Copy, Debug)]
    pub enum AudioCodecId {
        Opus,
        Vorbis,
    }

    impl AudioCodecId {
        fn get_id(&self) -> u32 {
            match self {
                AudioCodecId::Opus => ffi::mux::OPUS_CODEC_ID,
                AudioCodecId::Vorbis => ffi::mux::VORBIS_CODEC_ID,
            }
        }
    }

    #[derive(Eq, PartialEq, Clone, Copy, Debug)]
    pub enum VideoCodecId {
        VP8,
        VP9,
        AV1,
    }

    impl VideoCodecId {
        fn get_id(&self) -> u32 {
            match self {
                VideoCodecId::VP8 => ffi::mux::VP8_CODEC_ID,
                VideoCodecId::VP9 => ffi::mux::VP9_CODEC_ID,
                VideoCodecId::AV1 => ffi::mux::AV1_CODEC_ID,
            }
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
}
