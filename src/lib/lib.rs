#![feature(box_patterns)]

extern crate webm_sys as ffi;
extern crate libc;

pub mod mux {
    use ffi;
    use libc::{c_void, size_t};

    use std::io::{Write, Seek};
    use std::marker::PhantomData;

    pub struct Writer<T>
        where T: Write + Seek,
    {
        dest: Box<T>,
        mkv_writer: ffi::mux::WriterMutPtr,
    }
    impl<T> Writer<T>
        where T: Write + Seek,
    {
        pub fn new(dest: T) -> Writer<T> {
            use std::io::SeekFrom;
            use std::slice::from_raw_parts;
            use std::mem::transmute;
            let mut w = Writer {
                dest: Box::new(dest),
                mkv_writer: 0 as ffi::mux::WriterMutPtr,
            };

            extern "C" fn write_fn<T>(dest: *mut c_void,
                                      buf: *const c_void,
                                      len: size_t) -> bool
                where T: Write + Seek,
            {
                let dest: &mut T = unsafe { transmute(dest) };

                let buf = unsafe {
                    from_raw_parts(buf as *const u8, len as usize)
                };
                dest.write(buf).is_ok()
            }
            extern "C" fn get_pos_fn<T>(dest: *mut c_void) -> u64
                where T: Write + Seek,
            {
                let dest: &mut T = unsafe { transmute(dest) };
                dest.seek(SeekFrom::Current(0))
                    .unwrap()
            }
            extern "C" fn set_pos_fn<T>(dest: *mut c_void,
                                        pos: u64) -> bool
                where T: Write + Seek,
            {
                let dest: &mut T = unsafe { transmute(dest) };
                dest.seek(SeekFrom::Start(pos)).is_ok()
            }

            w.mkv_writer = unsafe {
                ffi::mux::new_writer(Some(write_fn::<T>),
                                     Some(get_pos_fn::<T>),
                                     Some(set_pos_fn::<T>),
                                     None,
                                     transmute(&mut *w.dest))
            };
            debug_assert!(w.mkv_writer != 0 as *mut _);
            w
        }

        fn mkv_writer(&self) -> ffi::mux::WriterMutPtr {
            self.mkv_writer
        }
        pub fn unwrap(self) -> T {
            unsafe {
                ffi::mux::delete_writer(self.mkv_writer);
            }
            let box dest = self.dest;
            dest
        }
    }

    #[derive(Eq, PartialEq, Clone, Copy)]
    pub struct VideoTrack<'a, 'b>(ffi::mux::SegmentMutPtr,
                                  ffi::mux::VideoTrackMutPtr,
                                  PhantomData<&'b Segment<'a>>)
        where 'a: 'b;
    #[derive(Eq, PartialEq, Clone, Copy)]
    pub struct AudioTrack<'a, 'b>(ffi::mux::SegmentMutPtr,
                                  ffi::mux::AudioTrackMutPtr,
                                  PhantomData<&'b Segment<'a>>)
        where 'a: 'b;
    pub trait Track {
        fn is_audio(&self) -> bool { false }
        fn is_video(&self) -> bool { false }

        fn add_frame(&mut self, data: &[u8], timestamp_ns: u64, keyframe: bool) -> bool {
            unsafe {
                ffi::mux::segment_add_frame(self.get_segment(),
                                            self.get_track(),
                                            data.as_ptr(),
                                            data.len() as size_t,
                                            timestamp_ns, keyframe)
            }
        }

        #[doc(hidden)]
        fn get_segment(&self) -> ffi::mux::SegmentMutPtr;

        #[doc(hidden)]
        fn get_track(&self) -> ffi::mux::TrackMutPtr;
    }
    impl<'a, 'b> Track for VideoTrack<'a, 'b>
        where 'a: 'b,
    {
        fn is_video(&self) -> bool { true }

        fn get_segment(&self) -> ffi::mux::SegmentMutPtr { self.0 }
        fn get_track(&self) -> ffi::mux::TrackMutPtr {
            unsafe { ffi::mux::video_track_base_mut(self.1) }
        }
    }
    impl<'a, 'b> Track for AudioTrack<'a, 'b>
        where 'a: 'b,
    {
        fn is_audio(&self) -> bool { true }

        fn get_segment(&self) -> ffi::mux::SegmentMutPtr { self.0 }
        fn get_track(&self) -> ffi::mux::TrackMutPtr {
            unsafe { ffi::mux::audio_track_base_mut(self.1) }
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
                &AudioCodecId::Opus => ffi::mux::OPUS_CODEC_ID,
                &AudioCodecId::Vorbis => ffi::mux::VORBIS_CODEC_ID,
            }
        }
    }
    #[derive(Eq, PartialEq, Clone, Copy, Debug)]
    pub enum VideoCodecId {
        VP8,
        VP9,
    }
    impl VideoCodecId {
        fn get_id(&self) -> u32 {
            match self {
                &VideoCodecId::VP8 => ffi::mux::VP8_CODEC_ID,
                &VideoCodecId::VP9 => ffi::mux::VP9_CODEC_ID,
            }
        }
    }

    pub struct Segment<'a> {
        ffi: ffi::mux::SegmentMutPtr,
        _dest: PhantomData<&'a u8>
    }

    impl<'a> Segment<'a> {
        pub fn new<T>(dest: &'a Writer<T>) -> Option<Segment<'a>>
            where T: Write + Seek,
        {
            let ffi = unsafe { ffi::mux::new_segment() };
            let success = unsafe {
                ffi::mux::initialize_segment(ffi, dest.mkv_writer())
            };
            if !success { return None; }

            Some(Segment {
                ffi: ffi,
                _dest: PhantomData,
            })
        }

        pub fn add_video_track<'b>(&'b mut self, width: i32, height: i32,
                                   id: Option<i32>, codec: VideoCodecId) -> VideoTrack<'a, 'b> {
            let vt = unsafe {
                ffi::mux::segment_add_video_track(self.ffi, width, height,
                                                  id.unwrap_or(0), codec.get_id())
            };
            VideoTrack(self.ffi, vt, PhantomData)
        }
        pub fn add_audio_track<'b>(&'b mut self, sample_rate: i32, channels: i32,
                                   id: Option<i32>, codec: AudioCodecId) -> AudioTrack<'a, 'b> {
            let at = unsafe {
                ffi::mux::segment_add_audio_track(self.ffi, sample_rate, channels,
                                                  id.unwrap_or(0), codec.get_id())
            };
            AudioTrack(self.ffi, at, PhantomData)
        }

        #[allow(unused_mut)]
        pub fn finalize(mut self) -> bool {
            let result = unsafe {
                ffi::mux::finalize_segment(self.ffi)
            };
            unsafe {
                ffi::mux::delete_segment(self.ffi);
            }
            result
        }
    }
}
