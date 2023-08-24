use std::ffi::CString;

use cef_sys::{cef_string_t, cef_string_utf8_to_utf16};

pub unsafe fn to_cef_str<S: ToString>(s: S) -> cef_string_t {
    let mut cstr = cef_string_t::default();
    let s = s.to_string();
    let bytes = s.as_bytes();
    let s = CString::new(bytes).expect("failed to convert to CString");
    cef_string_utf8_to_utf16(s.as_ptr(), s.as_bytes().len() as u64, &mut cstr);
    cstr
}

pub unsafe fn from_cef_str(s: *const cef_string_t) -> String {
    let bytes: &[u16] = std::slice::from_raw_parts((*s).str_, (*s).length as usize);
    String::from_utf16_lossy(bytes)
}
