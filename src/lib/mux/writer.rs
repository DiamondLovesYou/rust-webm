use std::ffi::c_void;
use std::io::{Seek, Write};
use std::marker::PhantomPinned;
use std::pin::Pin;
use std::ptr::NonNull;

use crate::ffi;
use crate::ffi::mux::{WriterGetPosFn, WriterSetPosFn};

/// RAII semantics for an FFI writer. This is simpler than implementing `Drop` on [`Writer`], which
/// prevents destructuring.
//
// SAFETY: `libwebm` does not contain thread-locals or anything that would violate `Send`-safety.
// `libwebm` is not thread-safe, however, which is why we do not implement `Sync`.
unsafe impl Send for OwnedWriterPtr {}

struct OwnedWriterPtr {
    writer: ffi::mux::WriterNonNullPtr,
}

impl OwnedWriterPtr {
    /// ## Safety
    /// `writer` must be a valid, non-dangling pointer to an FFI writer created with [`ffi::mux::new_writer`].
    /// After construction, `writer` must not be used by the caller, except via [`Self::as_ptr`].
    /// The latter also must not be passed to [`ffi::mux::delete_writer`].
    const unsafe fn new(writer: ffi::mux::WriterNonNullPtr) -> Self {
        Self { writer }
    }

    const fn as_ptr(&self) -> ffi::mux::WriterMutPtr {
        self.writer.as_ptr()
    }
}

impl Drop for OwnedWriterPtr {
    fn drop(&mut self) {
        // SAFETY: We are assumed to be the only one allowed to delete this writer (per the requirements of [`Self::new`]).
        unsafe {
            ffi::mux::delete_writer(self.writer.as_ptr());
        }
    }
}

/// Structure for writing a muxed WebM stream to the user-supplied write destination `T`.
///
/// `T` may be a file, an `std::io::Cursor` over a byte array, or anything implementing the [`Write`] trait.
/// It is recommended, but not required, that `T` also implement [`Seek`]. This allows the resulting WebM
/// file to have things like seeking headers and a stream duration known upfront.
///
/// Once this [`Writer`] is created, you can use it to create one or more [`Segment`](crate::mux::Segment)s.
pub struct Writer<T>
where
    T: Write,
{
    writer_data: Pin<Box<MuxWriterData<T>>>,
    mkv_writer: OwnedWriterPtr,
}

struct MuxWriterData<T> {
    dest: T,

    /// Used for tracking position when using a non-Seek write destination
    bytes_written: u64,
    _marker: PhantomPinned,
}

impl<T> Writer<T>
where
    T: Write,
{
    /// Creates a [`Writer`] for a destination that does not support [`Seek`].
    /// If it does support [`Seek`], you should use [`Writer::new()`] instead.
    #[inline]
    pub fn new_non_seek(dest: T) -> Self {
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
    ///
    /// It does not flush any unwritten data.
    #[must_use]
    #[inline]
    pub fn into_inner(self) -> T {
        let Self { writer_data, .. } = self;
        unsafe { Pin::into_inner_unchecked(writer_data).dest }
    }

    pub(crate) const fn mkv_writer(&self) -> ffi::mux::WriterMutPtr {
        self.mkv_writer.as_ptr()
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
            _marker: PhantomPinned,
        });
        let mkv_writer = unsafe {
            ffi::mux::new_writer(
                Some(write_fn::<T>),
                Some(get_pos_fn),
                set_pos_fn,
                None,
                std::ptr::from_mut(writer_data.as_mut().get_unchecked_mut()).cast(),
            )
        };
        assert!(!mkv_writer.is_null());

        Self {
            writer_data,
            mkv_writer: unsafe { OwnedWriterPtr::new(NonNull::new(mkv_writer).unwrap()) },
        }
    }
}

impl<T> Writer<T>
where
    T: Write + Seek,
{
    /// Creates a [`Writer`] for a destination that supports [`Seek`].
    /// If it does not support [`Seek`], you should use [`Writer::new_non_seek()`] instead.
    ///
    /// You can use `io::Cursor::new(Vec::new())` for in-memory writing, or `BufReader::new(File)`.
    #[inline]
    pub fn new(dest: T) -> Self {
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

#[test]
fn sendable() {
    fn is_send<T: Send>(_: &T) {}

    let w = Writer::new(std::io::Cursor::new(vec![1,2,3]));
    is_send(&w);
    assert_eq!([1,2,3], *w.into_inner().into_inner());

    let w = Writer::new_non_seek(vec![3,4,5]);
    is_send(&w);
    assert_eq!([3,4,5], *w.into_inner());
}
