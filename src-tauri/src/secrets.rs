#[cfg(windows)]
mod platform {
    use std::ptr::null_mut;
    use windows_sys::Win32::{Foundation::LocalFree, Security::Cryptography::{CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN}};
    pub fn transform(bytes: &[u8], protect: bool) -> Result<Vec<u8>, String> {
        if bytes.is_empty() { return Ok(vec![]); }
        let input = CRYPT_INTEGER_BLOB { cbData: bytes.len() as u32, pbData: bytes.as_ptr() as *mut u8 };
        let mut output = CRYPT_INTEGER_BLOB { cbData: 0, pbData: null_mut() };
        // DPAPI allocates the output buffer; copy it before freeing with LocalFree.
        let result = unsafe {
            if protect { CryptProtectData(&input, null_mut(), null_mut(), null_mut(), null_mut(), CRYPTPROTECT_UI_FORBIDDEN, &mut output) }
            else { CryptUnprotectData(&input, null_mut(), null_mut(), null_mut(), null_mut(), CRYPTPROTECT_UI_FORBIDDEN, &mut output) }
        };
        if result == 0 { return Err("Windows could not access the saved SteamGridDB credential. Re-enter your key in Settings.".into()); }
        let data = unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
        unsafe { LocalFree(output.pbData as *mut _); }
        Ok(data)
    }
}
pub fn encrypt(bytes: &[u8]) -> Result<Vec<u8>, String> {
    #[cfg(windows)] { platform::transform(bytes, true) }
    #[cfg(not(windows))] { if bytes.is_empty() { Ok(vec![]) } else { Err("Credential storage is currently supported on Windows only.".into()) } }
}
pub fn decrypt(bytes: &[u8]) -> Result<String, String> {
    #[cfg(windows)] { String::from_utf8(platform::transform(bytes, false)?).map_err(|e| e.to_string()) }
    #[cfg(not(windows))] { if bytes.is_empty() { Ok(String::new()) } else { Err("Cannot read Windows credentials on this platform.".into()) } }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    #[test]
    fn dpapi_roundtrip_protects_the_key() {
        let key = "arc-test-credential-only";
        let protected = encrypt(key.as_bytes()).unwrap();
        assert_ne!(protected, key.as_bytes());
        assert_eq!(decrypt(&protected).unwrap(), key);
    }
    #[test]
    fn corrupt_credentials_fail_without_returning_plaintext() {
        assert!(decrypt(b"not-a-dpapi-blob").is_err());
    }
}
