use std::path::Path;
#[derive(Debug, Default, Clone)]
pub struct ExeMetadata { pub product_name: Option<String>, pub description: Option<String> }

fn version_text(chars: &[u16]) -> Option<String> {
    // Some EXEs advertise a length extending into the next version-resource field.
    // Windows strings end at the first NUL, not at the end of that advertised range.
    let end = chars.iter().position(|char| *char == 0).unwrap_or(chars.len());
    let text = String::from_utf16_lossy(&chars[..end]).trim().to_owned();
    (!text.is_empty() && !text.chars().any(char::is_control)).then_some(text)
}

#[cfg(windows)]
pub fn read(path: &Path) -> ExeMetadata {
    use std::{ffi::c_void, os::windows::ffi::OsStrExt, ptr::null_mut};
    // Version APIs are read-only and work without loading or executing the PE file.
    #[link(name = "version")]
    extern "system" {
        fn GetFileVersionInfoSizeW(filename: *const u16, handle: *mut u32) -> u32;
        fn GetFileVersionInfoW(filename: *const u16, handle: u32, length: u32, data: *mut c_void) -> i32;
        fn VerQueryValueW(block: *const c_void, query: *const u16, value: *mut *mut c_void, length: *mut u32) -> i32;
    }
    let filename: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let size = unsafe { GetFileVersionInfoSizeW(filename.as_ptr(), null_mut()) };
    if size == 0 || size > 4 * 1024 * 1024 { return ExeMetadata::default(); }
    let mut bytes = vec![0u8; size as usize];
    if unsafe { GetFileVersionInfoW(filename.as_ptr(), 0, size, bytes.as_mut_ptr().cast()) } == 0 { return ExeMetadata::default(); }
    let query: Vec<u16> = "\\VarFileInfo\\Translation".encode_utf16().chain(Some(0)).collect();
    let mut pointer = null_mut(); let mut length = 0;
    let translations: Vec<(u16, u16)> = if unsafe { VerQueryValueW(bytes.as_ptr().cast(), query.as_ptr(), &mut pointer, &mut length) } != 0 && length >= 4 && !pointer.is_null() {
        unsafe { std::slice::from_raw_parts(pointer as *const u16, length as usize / 2) }.chunks_exact(2).map(|pair| (pair[0], pair[1])).collect()
    } else { vec![(0x0409, 0x04b0), (0x0409, 0x04e4)] };
    let get = |field: &str| -> Option<String> {
        for (language, page) in &translations {
            let query: Vec<u16> = format!("\\StringFileInfo\\{language:04x}{page:04x}\\{field}").encode_utf16().chain(Some(0)).collect();
            let mut value: *mut c_void = null_mut(); let mut chars = 0;
            if unsafe { VerQueryValueW(bytes.as_ptr().cast(), query.as_ptr(), &mut value, &mut chars) } != 0 && !value.is_null() && chars > 1 && chars < 4096 {
                let start = value as usize;
                let base = bytes.as_ptr() as usize;
                let end = base + bytes.len();
                if start < base || start >= end || start % 2 != 0 { continue; }
                let count = (chars as usize).min((end - start) / 2);
                if let Some(text) = version_text(unsafe { std::slice::from_raw_parts(value as *const u16, count) }) { return Some(text); }
            }
        }
        None
    };
    ExeMetadata { product_name: get("ProductName"), description: get("FileDescription") }
}
#[cfg(not(windows))]
pub fn read(_path: &Path) -> ExeMetadata { ExeMetadata::default() }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_resource_length_stops_at_first_nul() {
        let chars: Vec<_> = "Resident Evil 4\0\u{10}\u{1}FileVersion".encode_utf16().collect();
        assert_eq!(version_text(&chars).as_deref(), Some("Resident Evil 4"));
        assert!(version_text(&"bad\u{1}title".encode_utf16().collect::<Vec<_>>()).is_none());
    }
}
