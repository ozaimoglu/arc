//! Small read-only registry adapter; callers request only installation fields.
#[derive(Clone, Copy)]
pub enum Hive { Machine, User, Classes }

#[cfg(windows)]
fn root(hive: Hive) -> windows_sys::Win32::System::Registry::HKEY {
    use windows_sys::Win32::System::Registry::{HKEY_CLASSES_ROOT, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    match hive { Hive::Machine => HKEY_LOCAL_MACHINE, Hive::User => HKEY_CURRENT_USER, Hive::Classes => HKEY_CLASSES_ROOT }
}

pub fn string(hive: Hive, key: &str, name: &str) -> Option<String> {
    #[cfg(windows)] {
        use windows_sys::Win32::System::Registry::{RegGetValueW, RRF_RT_REG_SZ};
        let key: Vec<u16> = key.encode_utf16().chain(Some(0)).collect();
        let name: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        let mut buffer = vec![0_u16; 32768]; let mut size = (buffer.len() * 2) as u32;
        let status = unsafe { RegGetValueW(root(hive), key.as_ptr(), name.as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), buffer.as_mut_ptr().cast(), &mut size) };
        if status != 0 || size < 2 { return None; }
        let value = &buffer[..(size as usize / 2).min(buffer.len())];
        Some(String::from_utf16_lossy(&value[..value.iter().position(|char| *char == 0).unwrap_or(value.len())]))
    }
    #[cfg(not(windows))] { let _ = (hive, key, name); None }
}

pub fn subkeys(hive: Hive, key: &str) -> Vec<String> {
    #[cfg(windows)] {
        use windows_sys::Win32::System::Registry::{RegOpenKeyExW, RegEnumKeyExW, RegCloseKey, KEY_READ};
        let path: Vec<u16> = key.encode_utf16().chain(Some(0)).collect(); let mut handle = std::ptr::null_mut();
        if unsafe { RegOpenKeyExW(root(hive), path.as_ptr(), 0, KEY_READ, &mut handle) } != 0 { return vec![]; }
        let mut values = vec![];
        for index in 0..4096 {
            let mut name = [0_u16; 256]; let mut size = name.len() as u32;
            let status = unsafe { RegEnumKeyExW(handle, index, name.as_mut_ptr(), &mut size, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut()) };
            if status == 259 { break; }
            if status == 0 { values.push(String::from_utf16_lossy(&name[..size as usize])); }
        }
        unsafe { RegCloseKey(handle); }
        values
    }
    #[cfg(not(windows))] { let _ = (hive, key); vec![] }
}

pub fn executable_from_command(command: &str) -> Option<std::path::PathBuf> {
    let value = command.trim_start();
    let executable = if let Some(value) = value.strip_prefix('"') { value.split_once('"')?.0 }
        else { &value[..value.to_ascii_lowercase().find(".exe")? + 4] };
    Some(std::path::PathBuf::from(executable))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn handler_arguments_are_never_evaluated_or_forwarded() {
        for command in ["\"C:\\Program Files\\Client.exe\" \"%1\" --extra", "C:\\Program Files\\Client.exe \"%1\""] {
            assert_eq!(executable_from_command(command).unwrap(), std::path::Path::new("C:\\Program Files\\Client.exe"));
        }
        assert!(executable_from_command("\"unterminated.exe").is_none());
        assert!(executable_from_command("powershell -command anything").is_none());
    }
}
