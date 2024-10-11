use std::ptr::NonNull;

use ffi::mux::{ResultCode, TrackNum};

use super::{writer::MkvWriter, AudioCodecId, AudioTrack, Error, VideoCodecId, VideoTrack};

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

    /// Adds a new video track to this segment, returning its track number.
    ///
    /// You may request a specific track number using the `track_num` parameter. If one is specified, and this method
    /// succeeds, the returned track number is guaranteed to match the requested one. If a track with that number
    /// already exists, however, this method will fail. Leave as `None` to allow an available number to be chosen for
    /// you.
    ///
    /// This method will fail if called after the first frame has been written.
    pub fn add_video_track(
        &mut self,
        width: u32,
        height: u32,
        desired_track_num: Option<i32>,
        codec: VideoCodecId,
    ) -> Result<VideoTrack, Error> {
        let mut track_num_out: TrackNum = 0;
        let ffi = self.segment_ptr();
        let result = unsafe {
            ffi::mux::segment_add_video_track(
                ffi.as_ptr(),
                width as i32,
                height as i32,
                desired_track_num.unwrap_or(0),
                codec.get_id(),
                &mut track_num_out,
            )
        };

        match result {
            ResultCode::Ok => Ok(VideoTrack(track_num_out)),
            _ => Err(Error::Unknown),
        }
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

    /// Adds a new audio track to this segment, returning its track number.
    ///
    /// You may request a specific track number using the `track_num` parameter. If one is specified, and this method
    /// succeeds, the returned track number is guaranteed to match the requested one. If a track with that number
    /// already exists, however, this method will fail. Leave as `None` to allow an available number to be chosen for
    /// you.
    ///
    /// This method will fail if called after the first frame has been written.
    pub fn add_audio_track(
        &mut self,
        sample_rate: i32,
        channels: i32,
        desired_track_num: Option<i32>,
        codec: AudioCodecId,
    ) -> Result<AudioTrack, Error> {
        let mut track_num_out: TrackNum = 0;
        let ffi = self.segment_ptr();
        let result = unsafe {
            ffi::mux::segment_add_audio_track(
                ffi.as_ptr(),
                sample_rate,
                channels,
                desired_track_num.unwrap_or(0),
                codec.get_id(),
                &mut track_num_out,
            )
        };

        match result {
            ResultCode::Ok => Ok(AudioTrack(track_num_out)),
            _ => Err(Error::Unknown),
        }
    }

    /// Sets color information for the specified video track.
    ///
    /// This method will fail if called after the first frame has been written.
    pub fn set_color(
        &mut self,
        track: VideoTrack,
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
                track.into(),
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
    /// [`AudioTrack`], you can either pass it directly, or call `track_number()` to get the underlying [`TrackNum`].
    ///
    /// The timestamp must be in nanosecond units, and must be monotonically increasing with respect to all other
    /// timestamps written so far, including those of other tracks! Repeating the last written timestamp is allowed,
    /// however players generally don't handle this well if both such frames are on the same track.
    pub fn add_frame(
        &mut self,
        track: impl Into<TrackNum>,
        data: &[u8],
        timestamp_ns: u64,
        keyframe: bool,
    ) -> Result<(), Error> {
        let result = unsafe {
            ffi::mux::segment_add_frame(
                self.ffi.unwrap().as_ptr(),
                track.into(),
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

    /// Finalizes the segment and consumes it, returning the underlying writer. Note that the finalizing process will
    /// itself trigger writes (such as to write seeking information).
    ///
    /// The resulting WebM may not be playable if you drop the [`Segment`] without calling this first!
    ///
    /// You may specify an explicit `duration` to be written to the segment's `Duration` element. However, this requires
    /// seeking and thus will be ignored if the writer was not created with [`Seek`](std::io::Seek) support.
    ///
    /// Finalization is known to fail if no frames have been written.
    pub fn finalize(mut self, duration: Option<u64>) -> Result<W, W> {
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
