extern crate webm_sys as ffi;

pub mod mux {
    use crate::ffi;
    use std::os::raw::c_void;

    use std::io::{Write, Seek};
    use std::pin::Pin;
    use std::ptr::NonNull;

    pub struct Writer<T>
        where T: Write + Seek,
    {
        dest: Pin<Box<T>>,
        mkv_writer: ffi::mux::WriterNonNullPtr,
    }

    unsafe impl<T: Send + Write + Seek> Send for Writer<T> {}

    impl<T> Writer<T>
        where T: Write + Seek,
    {
        pub fn new(dest: T) -> Writer<T> {
            use std::io::SeekFrom;
            use std::slice::from_raw_parts;

            extern "C" fn write_fn<T>(dest: *mut c_void, buf: *const c_void, len: usize) -> bool
                where
                    T: Write + Seek,
            {
                if buf.is_null() {
                    return false;
                }
                let dest = unsafe { dest.cast::<T>().as_mut().unwrap() };
                let buf = unsafe { from_raw_parts(buf.cast::<u8>(), len) };
                dest.write(buf).is_ok()
            }
            extern "C" fn get_pos_fn<T>(dest: *mut c_void) -> u64
                where
                    T: Write + Seek,
            {
                let dest = unsafe { dest.cast::<T>().as_mut().unwrap() };
                dest.stream_position().unwrap()
            }
            extern "C" fn set_pos_fn<T>(dest: *mut c_void, pos: u64) -> bool
                where
                    T: Write + Seek,
            {
                let dest = unsafe { dest.cast::<T>().as_mut().unwrap() };
                dest.seek(SeekFrom::Start(pos)).is_ok()
            }

            let mut dest = Box::pin(dest);
            let mkv_writer = unsafe {
                ffi::mux::new_writer(Some(write_fn::<T>),
                                     Some(get_pos_fn::<T>),
                                     Some(set_pos_fn::<T>),
                                     None,
                                     (dest.as_mut().get_unchecked_mut() as *mut T).cast())
            };
            assert!(!mkv_writer.is_null());

            let w = Writer {
                dest,
                mkv_writer: NonNull::new(mkv_writer).unwrap(),
            };
            w
        }

        #[must_use]
        pub fn unwrap(self) -> T {
            unsafe {
                ffi::mux::delete_writer(self.mkv_writer.as_ptr());
                *Pin::into_inner_unchecked(self.dest)
            }
        }
    }

    #[doc(hidden)]
    pub trait MkvWriter {
        fn mkv_writer(&self) -> ffi::mux::WriterMutPtr;
    }

    impl<T> MkvWriter for Writer<T>
        where T: Write + Seek,
    {
        fn mkv_writer(&self) -> ffi::mux::WriterMutPtr {
            self.mkv_writer.as_ptr()
        }
    }

    #[derive(Eq, PartialEq, Clone, Copy)]
    pub struct VideoTrack(ffi::mux::SegmentNonNullPtr,
                          ffi::mux::VideoTrackNonNullPtr,
                          u64);

    #[derive(Eq, PartialEq, Clone, Copy)]
    pub struct AudioTrack(ffi::mux::SegmentNonNullPtr,
                          ffi::mux::AudioTrackNonNullPtr);

    unsafe impl Send for VideoTrack {}

    unsafe impl Send for AudioTrack {}

    pub trait Track {
        fn is_audio(&self) -> bool { false }
        fn is_video(&self) -> bool { false }

        fn add_frame(&mut self, data: &[u8], timestamp_ns: u64, keyframe: bool) -> bool {
            unsafe {
                ffi::mux::segment_add_frame(self.get_segment(),
                                            self.get_track(),
                                            data.as_ptr(),
                                            data.len(),
                                            timestamp_ns, keyframe)
            }
        }

        #[doc(hidden)]
        fn get_segment(&self) -> ffi::mux::SegmentMutPtr;

        #[doc(hidden)]
        fn get_track(&self) -> ffi::mux::TrackMutPtr;
    }

    impl VideoTrack {
        pub fn set_color(&mut self, bit_depth: u8, subsampling: (bool, bool), full_range: bool) -> bool {
            let (sampling_horiz, sampling_vert) = subsampling;
            fn to_int(b: bool) -> i32 { if b { 1 } else { 0 } }
            unsafe {
                ffi::mux::mux_set_color(self.get_track().cast(), bit_depth.into(), to_int(sampling_horiz), to_int(sampling_vert), to_int(full_range)) != 0
            }
        }

        #[must_use]
        pub fn track_number(&self) -> u64 {
            self.2
        }
    }

    impl Track for VideoTrack {
        fn is_video(&self) -> bool { true }

        #[doc(hidden)]
        fn get_segment(&self) -> ffi::mux::SegmentMutPtr { self.0.as_ptr() }
        #[doc(hidden)]
        fn get_track(&self) -> ffi::mux::TrackMutPtr {
            unsafe { ffi::mux::video_track_base_mut(self.1.as_ptr()) }
        }
    }

    impl Track for AudioTrack {
        fn is_audio(&self) -> bool { true }

        #[doc(hidden)]
        fn get_segment(&self) -> ffi::mux::SegmentMutPtr { self.0.as_ptr() }
        #[doc(hidden)]
        fn get_track(&self) -> ffi::mux::TrackMutPtr {
            unsafe { ffi::mux::audio_track_base_mut(self.1.as_ptr()) }
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

    pub struct Segment<W> {
        ffi: ffi::mux::SegmentNonNullPtr,
        writer: W,
    }

    impl<W> Segment<W> {
        /// Note: the supplied writer must have a lifetime larger than the segment.
        pub fn new(dest: W) -> Option<Self>
            where W: MkvWriter,
        {
            let ffi = unsafe { ffi::mux::new_segment() };
            let ffi = NonNull::new(ffi)?;
            let success = unsafe {
                ffi::mux::initialize_segment(ffi.as_ptr(), dest.mkv_writer())
            };
            if !success { return None; }

            Some(Segment {
                ffi,
                writer: dest,
            })
        }

        pub fn set_app_name(&mut self, name: &str) {
            let name = std::ffi::CString::new(name).unwrap();
            unsafe {
                ffi::mux::mux_set_writing_app(self.ffi.as_ptr(), name.as_ptr());
            }
        }

        pub fn add_video_track(&mut self, width: u32, height: u32,
                               id: Option<i32>, codec: VideoCodecId) -> VideoTrack
        {
            let mut id_out: u64 = 0;
            let vt = unsafe {
                ffi::mux::segment_add_video_track(self.ffi.as_ptr(), width as i32, height as i32,
                                                  id.unwrap_or(0), codec.get_id(), &mut id_out)
            };
            assert_ne!(vt, std::ptr::null_mut());
            let vt = NonNull::new(vt).unwrap();
            VideoTrack(self.ffi, vt, id_out)
        }
        pub fn add_video_track_opt(&mut self, width: u32, height: u32,
                                   id: Option<i32>, codec: VideoCodecId) -> Option<VideoTrack>
        {
            let mut id_out: u64 = 0;
            let vt = unsafe {
                ffi::mux::segment_add_video_track(self.ffi.as_ptr(), width as i32, height as i32,
                                                  id.unwrap_or(0), codec.get_id(), &mut id_out)
            };
            let vt = NonNull::new(vt)?;
            Some(VideoTrack(self.ffi, vt, id_out))
        }

        pub fn set_codec_private(&mut self, track_number: u64, data: &[u8]) -> bool {
            unsafe {
                ffi::mux::segment_set_codec_private(self.ffi.as_ptr(), track_number, data.as_ptr(), data.len().try_into().unwrap())
            }
        }

        pub fn add_audio_track(&mut self, sample_rate: i32, channels: i32,
                               id: Option<i32>, codec: AudioCodecId) -> AudioTrack {
            let at = unsafe {
                ffi::mux::segment_add_audio_track(self.ffi.as_ptr(), sample_rate, channels,
                                                  id.unwrap_or(0), codec.get_id())
            };
            assert_ne!(at, std::ptr::null_mut());
            let at = NonNull::new(at).unwrap();
            AudioTrack(self.ffi, at)
        }
        pub fn add_audio_track_opt(&mut self, sample_rate: i32, channels: i32,
                                   id: Option<i32>, codec: AudioCodecId) -> Option<AudioTrack> {
            let at = unsafe {
                ffi::mux::segment_add_audio_track(self.ffi.as_ptr(), sample_rate, channels,
                                                  id.unwrap_or(0), codec.get_id())
            };
            let at = NonNull::new(at)?;
            Some(AudioTrack(self.ffi, at))
        }

        pub fn try_finalize(self, duration: Option<u64>) -> Result<W, W> {
            let result = unsafe {
                ffi::mux::finalize_segment(self.ffi.as_ptr(), duration.unwrap_or(0))
            };
            unsafe {
                ffi::mux::delete_segment(self.ffi.as_ptr());
            }
            if result {
                Ok(self.writer)
            } else {
                Err(self.writer)
            }
        }

        /// After calling, all tracks are freed (ie you can't use them).
        pub fn finalize(self, duration: Option<u64>) -> bool {
            self.try_finalize(duration).is_ok()
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
        let video_track = segment.add_video_track_opt(420, 420, Some(123456), mux::VideoCodecId::VP8);
        assert!(video_track.is_none());
    }
}
