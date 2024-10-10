extern crate webm_sys as ffi;

pub mod mux {
    mod writer;

    pub use ffi::mux::TrackNum;
    pub use writer::Writer;

    use ffi::mux::ResultCode;
    use writer::MkvWriter;

    use crate::ffi;

    use std::ptr::NonNull;

    #[derive(Clone)]
    pub struct VideoTrack(TrackNum);

    #[derive(Clone)]
    pub struct AudioTrack(TrackNum);

    impl Eq for VideoTrack {}
    impl PartialEq for VideoTrack {
        fn eq(&self, track: &VideoTrack) -> bool {
            self.0 == track.0
        }
    }

    impl Eq for AudioTrack {}
    impl PartialEq for AudioTrack {
        fn eq(&self, track: &AudioTrack) -> bool {
            self.0 == track.0
        }
    }

    unsafe impl Send for VideoTrack {}

    unsafe impl Send for AudioTrack {}

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

        #[doc(hidden)]
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

    unsafe impl<W: Send> Send for Segment<W> {}

    /// The error type for this entire crate. More specific error types will
    /// be added in the future, hence the current marking as non-exhaustive.
    #[derive(Debug)]
    #[non_exhaustive]
    pub enum Error {
        /// An unknown error occurred. While this is typically the result of
        /// incorrect parameters to methods, this is not a guarantee.
        Unknown,
    }

    pub struct Segment<W> {
        ffi: Option<ffi::mux::SegmentNonNullPtr>,
        writer: Option<W>,
    }

    impl<W> Segment<W> {
        /// Note: the supplied writer must have a lifetime larger than the segment.
        pub fn new(dest: W) -> Option<Self>
        where
            W: MkvWriter,
        {
            let ffi = unsafe { ffi::mux::new_segment() };
            let ffi = NonNull::new(ffi)?;
            let result = unsafe { ffi::mux::initialize_segment(ffi.as_ptr(), dest.mkv_writer()) };
            if result != ResultCode::Ok {
                return None;
            }

            Some(Segment {
                ffi: Some(ffi),
                writer: Some(dest),
            })
        }

        fn segment_ptr(&self) -> ffi::mux::SegmentNonNullPtr {
            *self.ffi.as_ref().unwrap()
        }

        pub fn set_app_name(&mut self, name: &str) {
            let name = std::ffi::CString::new(name).unwrap();
            let ffi = self.segment_ptr();
            unsafe {
                ffi::mux::mux_set_writing_app(ffi.as_ptr(), name.as_ptr());
            }
        }

        pub fn add_video_track(
            &mut self,
            width: u32,
            height: u32,
            id: Option<i32>,
            codec: VideoCodecId,
        ) -> VideoTrack {
            let mut track_num_out: TrackNum = 0;
            let ffi = self.segment_ptr();
            let result = unsafe {
                ffi::mux::segment_add_video_track(
                    ffi.as_ptr(),
                    width as i32,
                    height as i32,
                    id.unwrap_or(0),
                    codec.get_id(),
                    &mut track_num_out,
                )
            };
            assert_eq!(result, ResultCode::Ok);
            VideoTrack(track_num_out)
        }

        pub fn add_video_track_opt(
            &mut self,
            width: u32,
            height: u32,
            id: Option<i32>,
            codec: VideoCodecId,
        ) -> Option<VideoTrack> {
            let mut track_num_out: TrackNum = 0;
            let ffi = self.segment_ptr();
            let result = unsafe {
                ffi::mux::segment_add_video_track(
                    ffi.as_ptr(),
                    width as i32,
                    height as i32,
                    id.unwrap_or(0),
                    codec.get_id(),
                    &mut track_num_out,
                )
            };
            if result != ResultCode::Ok {
                return None;
            }

            Some(VideoTrack(track_num_out))
        }

        pub fn set_codec_private(&mut self, track_number: u64, data: &[u8]) -> bool {
            let ffi = self.segment_ptr();
            unsafe {
                let result = ffi::mux::segment_set_codec_private(
                    ffi.as_ptr(),
                    track_number,
                    data.as_ptr(),
                    data.len().try_into().unwrap(),
                );
                result == ResultCode::Ok
            }
        }

        pub fn add_audio_track(
            &mut self,
            sample_rate: i32,
            channels: i32,
            id: Option<i32>,
            codec: AudioCodecId,
        ) -> AudioTrack {
            let mut track_num_out: TrackNum = 0;
            let ffi = self.segment_ptr();
            let result = unsafe {
                ffi::mux::segment_add_audio_track(
                    ffi.as_ptr(),
                    sample_rate,
                    channels,
                    id.unwrap_or(0),
                    codec.get_id(),
                    &mut track_num_out,
                )
            };
            assert_eq!(result, ResultCode::Ok);
            AudioTrack(track_num_out)
        }
        pub fn add_audio_track_opt(
            &mut self,
            sample_rate: i32,
            channels: i32,
            id: Option<i32>,
            codec: AudioCodecId,
        ) -> Option<AudioTrack> {
            let mut track_num_out: TrackNum = 0;
            let ffi = self.segment_ptr();
            let result = unsafe {
                ffi::mux::segment_add_audio_track(
                    ffi.as_ptr(),
                    sample_rate,
                    channels,
                    id.unwrap_or(0),
                    codec.get_id(),
                    &mut track_num_out,
                )
            };
            if result != ResultCode::Ok {
                return None;
            }
            Some(AudioTrack(track_num_out))
        }

        /// Sets color information for the specified video track.
        ///
        /// This method will fail if called after the first frame has been written.
        pub fn set_color(
            &mut self,
            track: &VideoTrack,
            bit_depth: u8,
            subsampling: (bool, bool),
            full_range: bool,
        ) -> Result<(), Error> {
            let (sampling_horiz, sampling_vert) = subsampling;
            fn to_int(b: bool) -> i32 {
                if b {
                    1
                } else {
                    0
                }
            }

            let result = unsafe {
                ffi::mux::mux_set_color(
                    self.ffi.unwrap().as_ptr(),
                    track.track_number(),
                    bit_depth.into(),
                    to_int(sampling_horiz),
                    to_int(sampling_vert),
                    to_int(full_range),
                )
            };

            match result {
                ResultCode::Ok => Ok(()),
                _ => Err(Error::Unknown),
            }
        }

        /// Adds a frame to the track with the specified track number. If you have a [`VideoTrack`] or
        /// [`AudioTrack`], you can call `track_number()` to get the underlying [`TrackNum`].
        ///
        /// The timestamp must be in nanosecond units, and must be monotonically increasing with respect to all other
        /// timestamps written so far, including those of other tracks! Repeating the last written timestamp is allowed,
        /// however players generally don't handle this well if both such frames are on the same track.
        pub fn add_frame(
            &mut self,
            track_num: TrackNum,
            data: &[u8],
            timestamp_ns: u64,
            keyframe: bool,
        ) -> Result<(), Error> {
            let result = unsafe {
                ffi::mux::segment_add_frame(
                    self.ffi.unwrap().as_ptr(),
                    track_num,
                    data.as_ptr(),
                    data.len(),
                    timestamp_ns,
                    keyframe,
                )
            };

            match result {
                ResultCode::Ok => Ok(()),
                _ => Err(Error::Unknown),
            }
        }

        pub fn try_finalize(mut self, duration: Option<u64>) -> Result<W, W> {
            let result = unsafe {
                let ffi = self.segment_ptr();
                ffi::mux::finalize_segment(ffi.as_ptr(), duration.unwrap_or(0))
            };
            let segment = self.ffi.take().unwrap();
            unsafe {
                ffi::mux::delete_segment(segment.as_ptr());
            }
            let writer = self.writer.take().unwrap();

            if result == ResultCode::Ok {
                Ok(writer)
            } else {
                Err(writer)
            }
        }

        /// After calling, all tracks are freed (ie you can't use them).
        pub fn finalize(self, duration: Option<u64>) -> bool {
            self.try_finalize(duration).is_ok()
        }
    }

    impl<W> Drop for Segment<W> {
        fn drop(&mut self) {
            if let Some(segment) = self.ffi.take() {
                unsafe {
                    ffi::mux::delete_segment(segment.as_ptr());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn bad_track_number() {
        let mut output = Vec::with_capacity(4_000_000); // 4 MB
        let writer = mux::Writer::new(Cursor::new(&mut output));
        let mut segment = mux::Segment::new(writer).expect("Segment should create OK");
        let video_track =
            segment.add_video_track_opt(420, 420, Some(123456), mux::VideoCodecId::VP8);
        assert!(video_track.is_none());
    }
}
