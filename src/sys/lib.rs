
pub mod mux {
    use std::os::raw::{c_void, c_char, c_int};
    use std::ptr::NonNull;

    pub type IWriter = c_void;
    pub type WriterMutPtr = *mut IWriter;
    pub type WriterNonNullPtr = NonNull<IWriter>;

    pub type WriterWriteFn = extern "C" fn(*mut c_void,
                                           *const c_void,
                                           usize) -> bool;
    pub type WriterGetPosFn = extern "C" fn(*mut c_void) -> u64;
    pub type WriterSetPosFn = extern "C" fn(*mut c_void, u64) -> bool;
    pub type WriterElementStartNotifyFn = extern "C" fn(*mut c_void, u64, i64);

    // audio
    pub const OPUS_CODEC_ID: u32 = 0;
    pub const VORBIS_CODEC_ID: u32 = 1;

    // video
    pub const VP8_CODEC_ID: u32 = 0;
    pub const VP9_CODEC_ID: u32 = 1;
    pub const AV1_CODEC_ID: u32 = 2;

    pub type Segment = c_void;
    pub type SegmentMutPtr = *mut Segment;
    pub type SegmentNonNullPtr = NonNull<Segment>;

    pub type Track = c_void;
    pub type TrackMutPtr = *mut Track;
    pub type TrackNonNullPtr = NonNull<Track>;

    pub type VideoTrack = c_void;
    pub type VideoTrackMutPtr = *mut VideoTrack;
    pub type VideoTrackNonNullPtr = NonNull<VideoTrack>;

    pub type AudioTrack = c_void;
    pub type AudioTrackMutPtr = *mut AudioTrack;
    pub type AudioTrackNonNullPtr = NonNull<AudioTrack>;


    #[link(name = "webmadapter", kind = "static")]
    extern "C" {
        #[link_name = "mux_new_writer"]
        pub fn new_writer(write: Option<WriterWriteFn>,
                          get_pos: Option<WriterGetPosFn>,
                          set_pos: Option<WriterSetPosFn>,
                          element_start_notify: Option<WriterElementStartNotifyFn>,
                          user_data: *mut c_void) -> WriterMutPtr;
        #[link_name = "mux_delete_writer"]
        pub fn delete_writer(writer: WriterMutPtr);

        #[link_name = "mux_new_segment"]
        pub fn new_segment() -> SegmentMutPtr;
        #[link_name = "mux_initialize_segment"]
        pub fn initialize_segment(segment: SegmentMutPtr, writer: WriterMutPtr) -> bool;
        pub fn mux_set_color(segment: VideoTrackMutPtr, bits: c_int, sampling_horiz: c_int, sampling_vert: c_int, full_range: c_int) -> c_int;
        pub fn mux_set_writing_app(segment: SegmentMutPtr, name: *const c_char);
        #[link_name = "mux_finalize_segment"]
        pub fn finalize_segment(segment: SegmentMutPtr, duration: u64) -> bool;
        #[link_name = "mux_delete_segment"]
        pub fn delete_segment(segment: SegmentMutPtr);

        #[link_name = "mux_video_track_base_mut"]
        pub fn video_track_base_mut(track: VideoTrackMutPtr) -> TrackMutPtr;
        #[link_name = "mux_audio_track_base_mut"]
        pub fn audio_track_base_mut(track: AudioTrackMutPtr) -> TrackMutPtr;

        #[link_name = "mux_segment_add_video_track"]
        pub fn segment_add_video_track(segment: SegmentMutPtr,
                                       width: i32, height: i32,
                                       number: i32, codec_id: u32, id_out: *const u64) -> VideoTrackMutPtr;
        #[link_name = "mux_segment_add_audio_track"]
        pub fn segment_add_audio_track(segment: SegmentMutPtr,
                                       sample_rate: i32, channels: i32,
                                       number: i32, codec_id: u32) -> AudioTrackMutPtr;
        #[link_name = "mux_segment_add_frame"]
        pub fn segment_add_frame(segment: SegmentMutPtr,
                                 track: TrackMutPtr,
                                 frame: *const u8, length: usize,
                                 timestamp_ns: u64, keyframe: bool) -> bool;
        #[link_name = "mux_segment_set_codec_private"]
        pub fn segment_set_codec_private(segment: SegmentMutPtr,
                                         number: u64, data: *const u8, len: i32) -> bool;
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
