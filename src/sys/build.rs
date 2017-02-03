extern crate gcc;

fn main() {
    let files = &[
        "libwebm/mkvmuxer.cpp",
        "libwebm/mkvmuxerutil.cpp",
        "libwebm/mkvparser.cpp",
        "libwebm/mkvreader.cpp",
        "libwebm/mkvwriter.cpp",
        "ffi.cpp",
    ];
    let mut c = gcc::Config::new();
    c.cpp(true);
    c.flag("-fno-rtti");
    c.flag("-std=gnu++11");
    c.flag("-fno-exceptions");
    for f in files.iter() {
        c.file(*f);
    }
    c.compile("libwebmadapter.a");
}
