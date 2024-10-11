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
        /// An unknown error occurred. While this is typically the result of
        /// incorrect parameters to methods, this is not a guarantee.
        Unknown,
    }
}
