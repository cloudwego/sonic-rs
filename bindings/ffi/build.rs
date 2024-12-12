use std::env;

use cbindgen::Language::C;

fn main() {
    setup_cbindgen();
}

fn setup_cbindgen() {
    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    cbindgen::Builder::new()
        .with_crate(crate_dir)
        .with_language(C)
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file("include/sonic_ffi.h");
}
