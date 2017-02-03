
extern crate libc;

pub mod mux {

    use libc::{c_void, size_t};

    pub type IWriter = c_void;
    pub type WriterMutPtr = *mut IWriter;

    pub type WriterWriteFn = extern "C" fn(*mut c_void,
                                           *const c_void,
                                           size_t) -> bool;
    pub type WriterGetPosFn = extern "C" fn(*mut c_void) -> u64;
    pub type WriterSetPosFn = extern "C" fn(*mut c_void, u64) -> bool;
    pub type WriterElementStartNotifyFn = extern "C" fn(*mut c_void, u64, i64);

    // audio
    pub const OPUS_CODEC_ID: u32 = 0;
    pub const VORBIS_CODEC_ID: u32 = 1;

    // video
    pub const VP8_CODEC_ID: u32 = 0;
    pub const VP9_CODEC_ID: u32 = 1;

    pub type Segment = c_void;
    pub type SegmentMutPtr = *mut Segment;

    pub type Track = c_void;
    pub type TrackMutPtr = *mut Track;

    pub type VideoTrack = c_void;
    pub type VideoTrackMutPtr = *mut VideoTrack;

    pub type AudioTrack = c_void;
    pub type AudioTrackMutPtr = *mut AudioTrack;


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
        #[link_name = "mux_finalize_segment"]
        pub fn finalize_segment(segment: SegmentMutPtr) -> bool;
        #[link_name = "mux_delete_segment"]
        pub fn delete_segment(segment: SegmentMutPtr);

        #[link_name = "mux_video_track_base_mut"]
        pub fn video_track_base_mut(track: VideoTrackMutPtr) -> TrackMutPtr;
        #[link_name = "mux_audio_track_base_mut"]
        pub fn audio_track_base_mut(track: AudioTrackMutPtr) -> TrackMutPtr;

        #[link_name = "mux_segment_add_video_track"]
        pub fn segment_add_video_track(segment: SegmentMutPtr,
                                       width: i32, height: i32,
                                       number: i32, codec_id: u32) -> VideoTrackMutPtr;
        #[link_name = "mux_segment_add_audio_track"]
        pub fn segment_add_audio_track(segment: SegmentMutPtr,
                                       sample_rate: i32, channels: i32,
                                       number: i32, codec_id: u32) -> AudioTrackMutPtr;
        #[link_name = "mux_segment_add_frame"]
        pub fn segment_add_frame(segment: SegmentMutPtr,
                                 track: TrackMutPtr,
                                 frame: *const u8, length: size_t,
                                 timestamp_ns: u64, keyframe: bool) -> bool;
    }
}
