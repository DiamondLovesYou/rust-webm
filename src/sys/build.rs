
extern crate pnacl_build_helper as helper;

pub fn main() {
    helper::set_pkg_config_envs();
    helper::print_lib_paths();

    let mut a = helper::Archive::new("webm");
    let args = &["-Os".to_string(),
                 "-fno-rtti".to_string(),
                 "-fno-exceptions".to_string(),
                 "-std=gnu++11".to_string(),
                 ];
    a.cxx("libwebm/mkvmuxer.cpp", args);
    a.cxx("libwebm/mkvmuxerutil.cpp", args);
    a.cxx("libwebm/mkvparser.cpp", args);
    a.cxx("libwebm/mkvreader.cpp", args);
    a.cxx("libwebm/mkvwriter.cpp", args);
    a.cxx("ffi.cpp", args);
    a.archive();
}
