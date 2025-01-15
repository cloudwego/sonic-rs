fn main() {
    cc::Build::new()
        .cpp(true)
        .flag("-march=haswell")
        .flag("-std=c++17")
        .flag("-O3")
        .include("thridparty/simdjson/singleheader")
        .include("thridparty/sonic-cpp/include")
        .file("wrapper/wrapper.cpp")
        .file("thridparty/simdjson/singleheader/simdjson.cpp")
        .cargo_metadata(true)
        .compile("cpp_wrapper");

    println!("cargo:rustc-link-lib=cpp_wrapper");
}
