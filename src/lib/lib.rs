extern crate webm_sys as ffi;

pub mod mux {
    use ffi::mux::{TrackNum, WriterGetPosFn, WriterSetPosFn};

    use crate::ffi;
    use std::os::raw::c_void;

    use std::io::{Seek, Write};
    use std::pin::Pin;
    use std::ptr::NonNull;
    use std::sync::{Arc, Mutex, MutexGuard, Weak};

    /// Structure for writing a muxed WebM stream to the user-supplied write destination `T`.
    ///
    /// `T` may be a file, an `std::io::Cursor` over a byte array, or anything implementing the [`Write`] trait.
    /// It is recommended, but not required, that `T` also implement [`Seek`]. This allows the resulting WebM
    /// file to have things like seeking headers and a stream duration known upfront.
    ///
    /// Once this [`Writer`] is created, you can use it to create one or more [`Segment`]s.
    pub struct Writer<T>
    where
        T: Write,
    {
        writer_data: Pin<Box<MuxWriterData<T>>>,
        mkv_writer: ffi::mux::WriterNonNullPtr,
    }

    unsafe impl<T: Send + Write> Send for Writer<T> {}

    struct MuxWriterData<T> {
        dest: T,

        /// Used for tracking position when using a non-Seek write destination
        bytes_written: u64,
    }

    impl<T> Writer<T>
    where
        T: Write,
    {
        /// Creates a [`Writer`] for a destination that does not support [`Seek`].
        /// If it does support [`Seek`], you should use [`Writer::new()`] instead.
        pub fn new_non_seek(dest: T) -> Writer<T> {
            extern "C" fn get_pos_fn<T>(data: *mut c_void) -> u64 {
                // The user-supplied writer does not track its own position.
                // Use our own based on how much has been written
                let data = unsafe { data.cast::<MuxWriterData<T>>().as_mut().unwrap() };
                data.bytes_written
            }

            Self::make_writer(dest, get_pos_fn::<T>, None)
        }

        /// Consumes this [`Writer`], and returns the user-supplied write destination
        /// that it was created with.
        #[must_use]
        pub fn unwrap(self) -> T {
            unsafe {
                ffi::mux::delete_writer(self.mkv_writer.as_ptr());
                Pin::into_inner_unchecked(self.writer_data).dest
            }
        }

        fn make_writer(
            dest: T,
            get_pos_fn: WriterGetPosFn,
            set_pos_fn: Option<WriterSetPosFn>,
        ) -> Self {
            extern "C" fn write_fn<T>(data: *mut c_void, buf: *const c_void, len: usize) -> bool
            where
                T: Write,
            {
                if buf.is_null() {
                    return false;
                }
                let data = unsafe { data.cast::<MuxWriterData<T>>().as_mut().unwrap() };
                let buf = unsafe { std::slice::from_raw_parts(buf.cast::<u8>(), len) };

                let result = data.dest.write(buf);
                if let Ok(num_bytes) = result {
                    // Guard against a future universe where sizeof(usize) > sizeof(u64)
                    let num_bytes_u64: u64 = num_bytes.try_into().unwrap();

                    data.bytes_written += num_bytes_u64;

                    // Partial writes are considered failure
                    num_bytes == len
                } else {
                    false
                }
            }

            let mut writer_data = Box::pin(MuxWriterData {
                dest,
                bytes_written: 0,
            });
            let mkv_writer = unsafe {
                ffi::mux::new_writer(
                    Some(write_fn::<T>),
                    Some(get_pos_fn),
                    set_pos_fn,
                    None,
                    (writer_data.as_mut().get_unchecked_mut() as *mut MuxWriterData<T>).cast(),
                )
            };
            assert!(!mkv_writer.is_null());

            Writer {
                writer_data,
                mkv_writer: NonNull::new(mkv_writer).unwrap(),
            }
        }
    }

    impl<T> Writer<T>
    where
        T: Write + Seek,
    {
        /// Creates a [`Writer`] for a destination that supports [`Seek`].
        /// If it does not support [`Seek`], you should use [`Writer::new_non_seek()`] instead.
        pub fn new(dest: T) -> Writer<T> {
            use std::io::SeekFrom;

            extern "C" fn get_pos_fn<T>(data: *mut c_void) -> u64
            where
                T: Write + Seek,
            {
                let data = unsafe { data.cast::<MuxWriterData<T>>().as_mut().unwrap() };
                data.dest.stream_position().unwrap()
            }
            extern "C" fn set_pos_fn<T>(data: *mut c_void, pos: u64) -> bool
            where
                T: Write + Seek,
            {
                let data = unsafe { data.cast::<MuxWriterData<T>>().as_mut().unwrap() };
                data.dest.seek(SeekFrom::Start(pos)).is_ok()
            }

            Self::make_writer(dest, get_pos_fn::<T>, Some(set_pos_fn::<T>))
        }
    }

    #[doc(hidden)]
    pub trait MkvWriter {
        fn mkv_writer(&self) -> ffi::mux::WriterMutPtr;
    }

    impl<T> MkvWriter for Writer<T>
    where
        T: Write,
    {
        fn mkv_writer(&self) -> ffi::mux::WriterMutPtr {
            self.mkv_writer.as_ptr()
        }
    }

    #[derive(Clone)]
    /// Clone only increments reference count, it's still one track
    pub struct VideoTrack(
        Weak<Mutex<ffi::mux::SegmentNonNullPtr>>,
        ffi::mux::VideoTrackNonNullPtr,
        TrackNum,
    );

    #[derive(Clone)]
    /// Clone only increments reference count, it's still one track
    pub struct AudioTrack(
        Weak<Mutex<ffi::mux::SegmentNonNullPtr>>,
        ffi::mux::AudioTrackNonNullPtr,
        TrackNum,
    );

    impl Eq for VideoTrack {}
    impl PartialEq for VideoTrack {
        fn eq(&self, track: &VideoTrack) -> bool {
            self.1 == track.1
        }
    }

    impl Eq for AudioTrack {}
    impl PartialEq for AudioTrack {
        fn eq(&self, track: &AudioTrack) -> bool {
            self.1 == track.1
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

        fn add_frame(&mut self, data: &[u8], timestamp_ns: u64, keyframe: bool) -> bool {
            unsafe {
                let segment = self.get_segment();
                let segment = segment.lock().unwrap();
                ffi::mux::segment_add_frame(
                    segment.as_ptr(),
                    self.track_number(),
                    data.as_ptr(),
                    data.len(),
                    timestamp_ns,
                    keyframe,
                )
            }
        }

        #[doc(hidden)]
        unsafe fn get_segment(&self) -> Arc<Mutex<ffi::mux::SegmentNonNullPtr>>;

        #[doc(hidden)]
        unsafe fn get_track(&self) -> ffi::mux::TrackMutPtr;

        #[must_use]
        fn track_number(&self) -> TrackNum;
    }

    impl VideoTrack {
        pub fn set_color(
            &mut self,
            bit_depth: u8,
            subsampling: (bool, bool),
            full_range: bool,
        ) -> bool {
            let (sampling_horiz, sampling_vert) = subsampling;
            fn to_int(b: bool) -> i32 {
                if b {
                    1
                } else {
                    0
                }
            }
            unsafe {
                ffi::mux::mux_set_color(
                    self.get_track().cast(),
                    bit_depth.into(),
                    to_int(sampling_horiz),
                    to_int(sampling_vert),
                    to_int(full_range),
                ) != 0
            }
        }
    }

    impl Track for VideoTrack {
        fn is_video(&self) -> bool {
            true
        }

        #[doc(hidden)]
        unsafe fn get_segment(&self) -> Arc<Mutex<ffi::mux::SegmentNonNullPtr>> {
            self.0.upgrade().expect("segment dropped before track")
        }

        #[doc(hidden)]
        unsafe fn get_track(&self) -> ffi::mux::TrackMutPtr {
            unsafe { ffi::mux::video_track_base_mut(self.1.as_ptr()) }
        }

        #[must_use]
        fn track_number(&self) -> TrackNum {
            self.2
        }
    }

    impl Track for AudioTrack {
        fn is_audio(&self) -> bool {
            true
        }

        #[doc(hidden)]
        unsafe fn get_segment(&self) -> Arc<Mutex<ffi::mux::SegmentNonNullPtr>> {
            self.0.upgrade().expect("segment dropped before track")
        }

        #[doc(hidden)]
        unsafe fn get_track(&self) -> ffi::mux::TrackMutPtr {
            unsafe { ffi::mux::audio_track_base_mut(self.1.as_ptr()) }
        }

        #[doc(hidden)]
        fn track_number(&self) -> TrackNum {
            self.2
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
        ffi: Option<Arc<Mutex<ffi::mux::SegmentNonNullPtr>>>,
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
            let success = unsafe { ffi::mux::initialize_segment(ffi.as_ptr(), dest.mkv_writer()) };
            if !success {
                return None;
            }

            Some(Segment {
                ffi: Some(Arc::new(Mutex::new(ffi))),
                writer: Some(dest),
            })
        }

        fn segment_ptr(&self) -> MutexGuard<ffi::mux::SegmentNonNullPtr> {
            self.ffi.as_ref().unwrap().lock().unwrap()
        }

        fn weak_segment_ptr(&self) -> Weak<Mutex<ffi::mux::SegmentNonNullPtr>> {
            Arc::downgrade(&self.ffi.as_ref().unwrap())
        }

        pub fn set_app_name(&mut self, name: &str) {
            let name = std::ffi::CString::new(name).unwrap();
            let ffi_lock = self.segment_ptr();
            unsafe {
                ffi::mux::mux_set_writing_app(ffi_lock.as_ptr(), name.as_ptr());
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
            let ffi_lock = self.segment_ptr();
            let vt = unsafe {
                ffi::mux::segment_add_video_track(
                    ffi_lock.as_ptr(),
                    width as i32,
                    height as i32,
                    id.unwrap_or(0),
                    codec.get_id(),
                    &mut track_num_out,
                )
            };
            assert_ne!(vt, std::ptr::null_mut());
            let vt = NonNull::new(vt).unwrap();
            VideoTrack(self.weak_segment_ptr(), vt, track_num_out)
        }

        pub fn add_video_track_opt(
            &mut self,
            width: u32,
            height: u32,
            id: Option<i32>,
            codec: VideoCodecId,
        ) -> Option<VideoTrack> {
            let mut track_num_out: TrackNum = 0;
            let ffi_lock = self.segment_ptr();
            let vt = unsafe {
                ffi::mux::segment_add_video_track(
                    ffi_lock.as_ptr(),
                    width as i32,
                    height as i32,
                    id.unwrap_or(0),
                    codec.get_id(),
                    &mut track_num_out,
                )
            };
            let vt = NonNull::new(vt)?;
            Some(VideoTrack(self.weak_segment_ptr(), vt, track_num_out))
        }

        pub fn set_codec_private(&mut self, track_number: u64, data: &[u8]) -> bool {
            let ffi_lock = self.segment_ptr();
            unsafe {
                ffi::mux::segment_set_codec_private(
                    ffi_lock.as_ptr(),
                    track_number,
                    data.as_ptr(),
                    data.len().try_into().unwrap(),
                )
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
            let ffi_lock = self.segment_ptr();
            let at = unsafe {
                ffi::mux::segment_add_audio_track(
                    ffi_lock.as_ptr(),
                    sample_rate,
                    channels,
                    id.unwrap_or(0),
                    codec.get_id(),
                    &mut track_num_out,
                )
            };
            assert_ne!(at, std::ptr::null_mut());
            let at = NonNull::new(at).unwrap();
            AudioTrack(self.weak_segment_ptr(), at, track_num_out)
        }
        pub fn add_audio_track_opt(
            &mut self,
            sample_rate: i32,
            channels: i32,
            id: Option<i32>,
            codec: AudioCodecId,
        ) -> Option<AudioTrack> {
            let mut track_num_out: TrackNum = 0;
            let ffi_lock = self.segment_ptr();
            let at = unsafe {
                ffi::mux::segment_add_audio_track(
                    ffi_lock.as_ptr(),
                    sample_rate,
                    channels,
                    id.unwrap_or(0),
                    codec.get_id(),
                    &mut track_num_out,
                )
            };
            let at = NonNull::new(at)?;
            Some(AudioTrack(self.weak_segment_ptr(), at, track_num_out))
        }

        pub fn try_finalize(mut self, duration: Option<u64>) -> Result<W, W> {
            let result = unsafe {
                let ffi_lock = self.segment_ptr();
                ffi::mux::finalize_segment(ffi_lock.as_ptr(), duration.unwrap_or(0))
            };
            // tracks have weak refs, so into_inner should work as long as there's no race condition
            let mut segment = Arc::into_inner(self.ffi.take().unwrap()).expect("segment is in use");
            unsafe {
                ffi::mux::delete_segment(segment.get_mut().unwrap().as_ptr());
            }
            let writer = self.writer.take().unwrap();

            if result {
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
            if let Some(mut segment) = self.ffi.take().and_then(Arc::into_inner) {
                if let Ok(ptr) = segment.get_mut() {
                    unsafe {
                        ffi::mux::delete_segment(ptr.as_ptr());
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mux::Track;
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

    #[test]
    #[should_panic = "segment dropped"]
    fn uaf() {
        let writer = crate::mux::Writer::new(std::io::Cursor::new(Vec::new()));
        let mut segment = crate::mux::Segment::new(writer).unwrap();
        let mut audio_track =
            segment.add_audio_track(48000, 1, None, crate::mux::AudioCodecId::Opus);

        drop(segment);
        audio_track.add_frame(&[], 0, true);
    }

    #[test]
    #[should_panic = "segment dropped"]
    fn finalized() {
        let writer = crate::mux::Writer::new(std::io::Cursor::new(Vec::new()));
        let mut segment = crate::mux::Segment::new(writer).unwrap();
        let mut audio_track =
            segment.add_audio_track(44000, 1, None, crate::mux::AudioCodecId::Vorbis);
        segment.finalize(None);
        audio_track.add_frame(&[], 0, true);
    }
}
