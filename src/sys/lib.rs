pub mod mux {
    use core::ffi::{c_char, c_void};
    use core::ptr::NonNull;

    #[repr(C)]
    pub struct IWriter {
        _opaque_c_aligned: *mut c_void,
    }
    pub type WriterMutPtr = *mut IWriter;
    pub type WriterNonNullPtr = NonNull<IWriter>;

    pub type WriterWriteFn = extern "C" fn(*mut c_void, *const c_void, usize) -> bool;
    pub type WriterGetPosFn = extern "C" fn(*mut c_void) -> u64;
    pub type WriterSetPosFn = extern "C" fn(*mut c_void, u64) -> bool;
    pub type WriterElementStartNotifyFn = extern "C" fn(*mut c_void, u64, i64);

    /// An opaque number used to identify an added track.
    pub type TrackNum = u64;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[repr(i32)]
    pub enum ResultCode {
        /// The function completed without error
        Ok = 0,

        /// An invalid parameter was passed (e.g. a null pointer or an invalid track number)
        BadParam = -1,

        /// `libwebm` returned an error, and no more specific error info is known. No assumptions
        /// should be made about whether this is an issue with the caller, or something internal
        /// to `libwebm`.
        UnknownLibwebmError = -2,
    }

    // audio
    pub const OPUS_CODEC_ID: u32 = 0;
    pub const VORBIS_CODEC_ID: u32 = 1;

    // video
    pub const VP8_CODEC_ID: u32 = 0;
    pub const VP9_CODEC_ID: u32 = 1;
    pub const AV1_CODEC_ID: u32 = 2;

    #[repr(C)]
    pub struct Segment {
        _opaque_c_aligned: *mut c_void,
    }
    pub type SegmentMutPtr = *mut Segment;
    pub type SegmentNonNullPtr = NonNull<Segment>;

    #[link(name = "webmadapter", kind = "static")]
    extern "C" {
        #[link_name = "mux_new_writer"]
        pub fn new_writer(
            write: Option<WriterWriteFn>,
            get_pos: Option<WriterGetPosFn>,
            set_pos: Option<WriterSetPosFn>,
            element_start_notify: Option<WriterElementStartNotifyFn>,
            user_data: *mut c_void,
        ) -> WriterMutPtr;
        #[link_name = "mux_delete_writer"]
        pub fn delete_writer(writer: WriterMutPtr);

        #[link_name = "mux_new_segment"]
        pub fn new_segment() -> SegmentMutPtr;
        #[link_name = "mux_initialize_segment"]
        pub fn initialize_segment(segment: SegmentMutPtr, writer: WriterMutPtr) -> ResultCode;
        #[link_name = "mux_set_color"]
        pub fn mux_set_color(
            segment: SegmentMutPtr,
            video_track_num: TrackNum,
            bits: u64,
            sampling_horiz: u64,
            sampling_vert: u64,
            color_range: u64,
        ) -> ResultCode;
        #[link_name = "mux_set_writing_app"]
        pub fn mux_set_writing_app(segment: SegmentMutPtr, name: *const c_char);
        #[link_name = "mux_finalize_segment"]
        pub fn finalize_segment(segment: SegmentMutPtr, duration: u64) -> ResultCode;
        #[link_name = "mux_delete_segment"]
        pub fn delete_segment(segment: SegmentMutPtr);

        #[link_name = "mux_segment_add_video_track"]
        pub fn segment_add_video_track(
            segment: SegmentMutPtr,
            width: i32,
            height: i32,
            number: i32,
            codec_id: u32,
            track_num_out: *mut TrackNum,
        ) -> ResultCode;
        #[link_name = "mux_segment_add_audio_track"]
        pub fn segment_add_audio_track(
            segment: SegmentMutPtr,
            sample_rate: i32,
            channels: i32,
            number: i32,
            codec_id: u32,
            track_num_out: *mut TrackNum,
        ) -> ResultCode;
        #[link_name = "mux_segment_add_frame"]
        pub fn segment_add_frame(
            segment: SegmentMutPtr,
            track_num: TrackNum,
            frame: *const u8,
            length: usize,
            timestamp_ns: u64,
            keyframe: bool,
        ) -> ResultCode;
        #[link_name = "mux_segment_set_codec_private"]
        pub fn segment_set_codec_private(
            segment: SegmentMutPtr,
            track_num: TrackNum,
            data: *const u8,
            len: i32,
        ) -> ResultCode;
    }
}

#[test]
fn smoke_test() {
    unsafe {
        let segment = mux::new_segment();
        assert!(!segment.is_null());
        mux::delete_segment(segment);
    }
}
