extern crate cc;

fn main() {
    let files = &[
        "libwebm/mkvmuxer/mkvmuxer.cc",
        "libwebm/mkvmuxer/mkvwriter.cc",
        "libwebm/mkvmuxer/mkvmuxerutil.cc",
        "libwebm/mkvparser/mkvparser.cc",
        "libwebm/mkvparser/mkvreader.cc",
        "ffi.cpp",
    ];
    let mut c = cc::Build::new();
    c.cpp(true);
    c.warnings(false);
    c.flag("-fno-rtti");
    c.flag("-std=gnu++11");
    c.flag("-fno-exceptions");
    c.include("libwebm");
    for &f in files.iter() {
        c.file(f);
    }
    c.compile("libwebmadapter.a");
}
