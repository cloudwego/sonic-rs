use std::ffi::c_char;

extern "C" {
    fn simdjson_cpp_parse_dom(_: *const c_char, _: usize) -> bool;
    fn sonic_cpp_parse_dom(_: *const c_char, _: usize) -> bool;

}

pub fn simdjson_cpp_binding_parse_dom(data: &[u8]) -> bool {
    unsafe {
        let len = data.len();
        let data = data.as_ptr() as *const c_char;
        simdjson_cpp_parse_dom(data, len)
    }
}

pub fn sonic_cpp_binding_parse_dom(data: &[u8]) -> bool {
    unsafe {
        let len = data.len();
        let data = data.as_ptr() as *const c_char;
        sonic_cpp_parse_dom(data, len)
    }
}
