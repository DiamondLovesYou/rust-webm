use std::{io::Write, ptr::NonNull};

use ffi::mux::{ResultCode, TrackNum};

use super::{
    writer::Writer, AudioCodecId, AudioTrack, ColorRange, ColorSubsampling, Error, VideoCodecId,
    VideoTrack,
};

/// RAII semantics for an FFI segment. This is simpler than implementing `Drop` on [`Segment`], which
/// prevents destructuring.
struct OwnedSegmentPtr {
    segment: ffi::mux::SegmentNonNullPtr,
}

impl OwnedSegmentPtr {
    /// ## Safety
    /// `segment` must be a valid, non-dangling pointer to an FFI segment created with [`ffi::mux::new_segment`].
    /// After construction, `segment` must not be used by the caller, except via [`Self::as_ptr`].
    /// The latter also must not be passed to [`ffi::mux::delete_segment`].
    unsafe fn new(segment: ffi::mux::SegmentNonNullPtr) -> Self {
        Self { segment }
    }

    fn as_ptr(&self) -> ffi::mux::SegmentMutPtr {
        self.segment.as_ptr()
    }
}

impl Drop for OwnedSegmentPtr {
    fn drop(&mut self) {
        // SAFETY: We are assumed to be the only one allowed to delete this segment (per the requirements of [`Self::new`]).
        unsafe {
            ffi::mux::delete_segment(self.segment.as_ptr());
        }
    }
}

/// A Matroska segment. This is where tracks are created and frames are written.
///
/// In typical usage, you first create a [`Writer`](crate::mux::Writer), use that to create a single segment, and go
/// from there.
///
/// ## Finalization
/// Once you are done writing frames to this segment, you must call [`Segment::finalize`] on it.
/// This performs a few final writes, and the resulting WebM may not be playable without it.
/// Notably, for memory safety reasons, just dropping a [`Segment`] will not finalize it!
pub struct Segment<W: Write> {
    ffi: OwnedSegmentPtr,
    writer: Writer<W>,
}

// SAFETY: `libwebm` does not contain thread-locals or anything that would violate `Send`-safety.
// Thus, safety is only conditional on the write destination `W`, hence the `Send` bound on it.
//
// `libwebm` is not thread-safe, however, which is why we do not implement `Sync`.
unsafe impl<W: Write + Send> Send for Segment<W> {}

impl<W: Write> Segment<W> {
    /// Creates a new Matroska segment that writes WebM data to `dest`.
    pub fn new(dest: Writer<W>) -> Result<Self, Error> {
        let ffi = unsafe { ffi::mux::new_segment() };
        let ffi = NonNull::new(ffi).ok_or(Error::Unknown)?;
        let result = unsafe { ffi::mux::initialize_segment(ffi.as_ptr(), dest.mkv_writer()) };

        match result {
            ResultCode::Ok => {
                let ffi = unsafe { OwnedSegmentPtr::new(ffi) };
                Ok(Segment { ffi, writer: dest })
            }
            _ => {
                unsafe {
                    ffi::mux::delete_segment(ffi.as_ptr());
                }
                Err(Error::Unknown)
            }
        }
    }

    /// Sets the name of the muxing application. This will become the `MuxingApp` element of the resulting
    /// WebM.
    ///
    /// Calling this after the first frame has been written has no effect.
    pub fn set_app_name(&mut self, name: &str) {
        let name = std::ffi::CString::new(name).unwrap();
        unsafe {
            ffi::mux::mux_set_writing_app(self.ffi.as_ptr(), name.as_ptr());
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
        desired_track_num: Option<TrackNum>,
        codec: VideoCodecId,
    ) -> Result<VideoTrack, Error> {
        let mut track_num_out: TrackNum = 0;

        // Zero is not a valid track number, and to libwebm means "choose one for me".
        // If this is the user's intent, they should instead pass `None`.
        if desired_track_num == Some(0) {
            return Err(Error::BadParam);
        }

        // libwebm requires i32 for these
        let width: i32 = try_as_i32(width)?;
        let height: i32 = try_as_i32(height)?;
        if width == 0 || height == 0 {
            return Err(Error::BadParam);
        }
        let requested_track_num: i32 = try_as_i32(desired_track_num.unwrap_or(0))?;

        let result = unsafe {
            ffi::mux::segment_add_video_track(
                self.ffi.as_ptr(),
                width,
                height,
                requested_track_num,
                codec.get_id(),
                &mut track_num_out,
            )
        };

        match result {
            ResultCode::Ok => {
                // If a specific track number was requested, make sure we got it
                if let Some(desired) = desired_track_num {
                    assert_eq!(desired, track_num_out);
                }

                Ok(VideoTrack(track_num_out))
            }
            _ => Err(Error::Unknown),
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
        sample_rate: u32,
        channels: u32,
        desired_track_num: Option<TrackNum>,
        codec: AudioCodecId,
    ) -> Result<AudioTrack, Error> {
        let mut track_num_out: TrackNum = 0;

        // Zero is not a valid track number, and to libwebm means "choose one for me".
        // If this is the user's intent, they should instead pass `None`.
        if desired_track_num == Some(0) {
            return Err(Error::BadParam);
        }

        // libwebm requires i32 for these
        let sample_rate: i32 = try_as_i32(sample_rate)?;
        let channels: i32 = try_as_i32(channels)?;
        if sample_rate == 0 || channels == 0 {
            return Err(Error::BadParam);
        }
        let requested_track_num: i32 = try_as_i32(desired_track_num.unwrap_or(0))?;

        let result = unsafe {
            ffi::mux::segment_add_audio_track(
                self.ffi.as_ptr(),
                sample_rate,
                channels,
                requested_track_num,
                codec.get_id(),
                &mut track_num_out,
            )
        };

        match result {
            ResultCode::Ok => {
                // If a specific track number was requested, make sure we got it
                if let Some(desired) = desired_track_num {
                    assert_eq!(desired, track_num_out);
                }

                Ok(AudioTrack(track_num_out))
            }
            _ => Err(Error::Unknown),
        }
    }

    /// Sets the CodecPrivate data a frame to the specified track. If you have a [`VideoTrack`] or [`AudioTrack`], you
    /// can either pass it directly, or call `track_number()` to get the underlying [`TrackNum`].
    ///
    /// This method will fail if called after the first frame has been written.
    pub fn set_codec_private(
        &mut self,
        track: impl Into<TrackNum>,
        data: &[u8],
    ) -> Result<(), Error> {
        unsafe {
            let result = ffi::mux::segment_set_codec_private(
                self.ffi.as_ptr(),
                track.into(),
                data.as_ptr(),
                data.len().try_into().unwrap(),
            );

            match result {
                ResultCode::Ok => Ok(()),
                _ => Err(Error::Unknown),
            }
        }
    }

    /// Sets color information for the specified video track.
    ///
    /// This method will fail if called after the first frame has been written.
    pub fn set_color(
        &mut self,
        track: VideoTrack,
        bit_depth: u8,
        subsampling: ColorSubsampling,
        color_range: ColorRange,
    ) -> Result<(), Error> {
        let color_range = match color_range {
            ColorRange::Unspecified => 0,
            ColorRange::Broadcast => 1,
            ColorRange::Full => 2,
        };

        let result = unsafe {
            ffi::mux::mux_set_color(
                self.ffi.as_ptr(),
                track.into(),
                bit_depth.into(),
                subsampling.chroma_horizontal,
                subsampling.chroma_vertical,
                color_range,
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
                self.ffi.as_ptr(),
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
    pub fn finalize(self, duration: Option<u64>) -> Result<Writer<W>, Writer<W>> {
        let Self { ffi, writer } = self;
        let result = unsafe { ffi::mux::finalize_segment(ffi.as_ptr(), duration.unwrap_or(0)) };

        match result {
            ResultCode::Ok => Ok(writer),
            _ => Err(writer),
        }
    }
}

fn try_as_i32(x: impl TryInto<i32>) -> Result<i32, Error> {
    x.try_into().map_err(|_| Error::BadParam)
}

#[cfg(test)]
mod tests {
    use crate::mux::Writer;

    use super::*;
    use std::io::Cursor;

    #[test]
    fn bad_track_number() {
        let mut output = Vec::new();
        let writer = Writer::new(Cursor::new(&mut output));
        let mut segment = Segment::new(writer).expect("Segment should create OK");
        let video_track = segment.add_video_track(420, 420, Some(123456), VideoCodecId::VP8);
        assert!(video_track.is_err());
    }

    #[test]
    fn overlapping_track_number() {
        let mut output = Vec::new();
        let writer = Writer::new(Cursor::new(&mut output));
        let mut segment = Segment::new(writer).expect("Segment should create OK");

        let video_track = segment.add_video_track(420, 420, Some(123), VideoCodecId::VP8);
        assert!(video_track.is_ok());

        let video_track2 = segment.add_video_track(420, 420, Some(123), VideoCodecId::VP8);
        assert!(video_track2.is_err());

        let audio_track = segment.add_audio_track(420, 420, Some(123), AudioCodecId::Opus);
        assert!(audio_track.is_err());
    }
}
