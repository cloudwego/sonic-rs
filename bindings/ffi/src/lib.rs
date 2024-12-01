use std::{ffi::c_char, mem::ManuallyDrop, os::raw::c_void};

use sonic_rs::Value;

/// A string allocated in Rust, ending with `\0`. Used for serialize output and error message.
#[derive(Debug)]
#[repr(C)]
pub struct SonicCString {
    buf: *const c_void,
    len: usize,
}

impl Default for SonicCString {
    fn default() -> Self {
        SonicCString {
            buf: std::ptr::null(),
            len: 0,
        }
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct SonicDeserializeRet {
    value: *const c_void,
    err: SonicCString,
}

pub const SONIC_RS_DESERIALIZE_USE_RAW: u64 = 1;
pub const SONIC_RS_DESERIALIZE_USE_RAWNUMBER: u64 = 2;
pub const SONIC_RS_DESERIALIZE_UTF8_LOSSY: u64 = 4;

/// # Safety
///
/// The caller should drop the returned `value` or `err`.
#[no_mangle]
pub unsafe extern "C" fn sonic_rs_deserialize_value(
    json: *const c_char,
    len: usize,
    cfg: u64,
) -> SonicDeserializeRet {
    let json = std::slice::from_raw_parts(json as *const u8, len);
    let mut de = sonic_rs::serde::Deserializer::from_slice(json);

    if cfg & SONIC_RS_DESERIALIZE_USE_RAWNUMBER != 0 {
        de = de.use_rawnumber();
    }

    if cfg & SONIC_RS_DESERIALIZE_USE_RAW != 0 {
        de = de.use_raw();
    }

    if cfg & SONIC_RS_DESERIALIZE_UTF8_LOSSY != 0 {
        de = de.utf8_lossy();
    }

    match de.deserialize::<Value>() {
        Ok(value) => SonicDeserializeRet {
            value: Box::into_raw(Box::new(value)) as *const _,
            err: SonicCString::default(),
        },
        Err(e) => {
            // messega always end with '\0'
            let msg = ManuallyDrop::new(format!("{}\0", e));
            let err = SonicCString {
                buf: msg.as_ptr() as *const c_void,
                len: msg.len(),
            };
            SonicDeserializeRet {
                value: std::ptr::null_mut(),
                err,
            }
        }
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct SonicSerializeRet {
    json: SonicCString,
    err: SonicCString,
}

pub const SONIC_RS_SERIALIZE_PRETTY: u64 = 1;

/// # Safety
///
/// The caller should drop the returned `json` or `err`.
#[no_mangle]
pub unsafe extern "C" fn sonic_rs_serialize_value(
    value: *const c_void,
    cfg: u64,
) -> SonicSerializeRet {
    let value = unsafe { &*(value as *const Value) };
    let ret = if cfg & SONIC_RS_SERIALIZE_PRETTY != 0 {
        sonic_rs::to_string_pretty(value)
    } else {
        sonic_rs::to_string(value)
    };

    match ret {
        Ok(json) => {
            let json = ManuallyDrop::new(json);
            let json = SonicCString {
                buf: json.as_ptr() as *const c_void,
                len: json.len(),
            };
            SonicSerializeRet {
                json,
                err: SonicCString::default(),
            }
        }
        Err(e) => {
            // NOTE: should be dropped manually in the foreign caller
            let msg = ManuallyDrop::new(format!("{}\0", e));
            let err = SonicCString {
                buf: msg.as_ptr() as *const c_void,
                len: msg.len(),
            };
            SonicSerializeRet {
                json: SonicCString::default(),
                err,
            }
        }
    }
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn sonic_rs_drop_value(value: *mut c_void) {
    std::mem::drop(Box::from_raw(value as *mut Value));
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn sonic_rs_drop_string(buf: *mut u8, len: u64) {
    let buf = Vec::<u8>::from_raw_parts(buf, len as usize, len as usize);
    std::mem::drop(buf);
}
