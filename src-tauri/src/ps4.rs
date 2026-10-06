use std::{collections::HashMap, io::Read, path::{Path, PathBuf}};
use crate::{models::{path_string, Candidate, ConsoleLaunch}, scanner::clean_title};

pub const SOURCE: &str = "PS4 · shadPS4";
const MAX_METADATA: usize = 128 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub struct Metadata { pub title: String, pub title_id: String }

fn u32_at(bytes: &[u8], offset: usize) -> Option<usize> {
    Some(u32::from_le_bytes(bytes.get(offset..offset.checked_add(4)?)?.try_into().ok()?) as usize)
}

pub fn parse_sfo(bytes: &[u8]) -> Option<Metadata> {
    if bytes.len() > MAX_METADATA || bytes.get(..4)? != b"\0PSF" || ![0x100, 0x101].contains(&u32_at(bytes, 4)?) { return None; }
    let keys = u32_at(bytes, 8)?; let values = u32_at(bytes, 12)?; let count = u32_at(bytes, 16)?;
    if count > 512 || keys < 20_usize.checked_add(count.checked_mul(16)?)? || values < keys || values > bytes.len() { return None; }
    let mut fields = HashMap::new();
    for index in 0..count {
        let offset = 20 + index * 16;
        let key_offset = u16::from_le_bytes(bytes.get(offset..offset + 2)?.try_into().ok()?) as usize;
        let format = u16::from_le_bytes(bytes.get(offset + 2..offset + 4)?.try_into().ok()?);
        let length = u32_at(bytes, offset + 4)?;
        if length > u32_at(bytes, offset + 8)? { return None; }
        let key_data = bytes.get(keys.checked_add(key_offset)?..values)?;
        let key = std::str::from_utf8(key_data.get(..key_data.iter().position(|byte| *byte == 0)?)?).ok()?;
        let start = values.checked_add(u32_at(bytes, offset + 12)?)?;
        let data = bytes.get(start..start.checked_add(length)?)?;
        if format == 0x0204 && ["TITLE", "TITLE_ID", "CATEGORY"].contains(&key) {
            let end = data.iter().position(|byte| *byte == 0)?;
            if fields.insert(key, std::str::from_utf8(&data[..end]).ok()?).is_some() { return None; }
        }
    }
    // Only base applications become library entries. Patches, DLC and saves
    // carry their own SFOs, often with the same title and serial.
    if fields.get("CATEGORY")? != &"gd" { return None; }
    let title_id = fields.get("TITLE_ID")?.to_string();
    if title_id.len() != 9 || !title_id.starts_with("CUSA") || !title_id[4..].bytes().all(|byte| byte.is_ascii_digit()) { return None; }
    let title = clean_title(fields.get("TITLE")?);
    if title.is_empty() || title.len() > 800 { return None; }
    Some(Metadata { title, title_id })
}

fn bounded_read(path: &Path) -> Option<Vec<u8>> {
    let mut bytes = vec![];
    std::fs::File::open(path).ok()?.take((MAX_METADATA + 1) as u64).read_to_end(&mut bytes).ok()?;
    (bytes.len() <= MAX_METADATA).then_some(bytes)
}

pub fn metadata(eboot: &Path) -> Option<Metadata> {
    if !eboot.file_name()?.to_string_lossy().eq_ignore_ascii_case("eboot.bin") || !eboot.is_file() { return None; }
    parse_sfo(&bounded_read(&eboot.parent()?.join("sce_sys/param.sfo"))?)
}

pub fn bloodborne_configuration(launcher: &Path) -> Option<(PathBuf, PathBuf)> {
    let config = bounded_read(&launcher.parent()?.join("BBLauncher/LauncherSettings.toml"))?;
    let config: toml::Value = toml::from_str(std::str::from_utf8(&config).ok()?).ok()?;
    let config = config.get("Launcher")?;
    let install = std::fs::canonicalize(config.get("installPath")?.as_str()?).ok()?;
    let emulator = std::fs::canonicalize(config.get("shadPath-New")?.as_str()?).ok()?;
    if !install.is_dir() || !emulator.is_file() || !emulator.file_name()?.to_string_lossy().eq_ignore_ascii_case("shadPS4.exe") { return None; }
    Some((install, emulator))
}

pub fn candidate(eboot: &Path) -> Option<Candidate> {
    let metadata = metadata(eboot)?;
    let folder = eboot.parent()?;
    let mut launch = None;
    for ancestor in folder.ancestors().take(8) {
        if metadata.title.eq_ignore_ascii_case("Bloodborne") {
            let launcher = ancestor.join("Bloodborne/Launcher/BB_Launcher.exe");
            if launcher.is_file() {
                if let Some((install, emulator)) = bloodborne_configuration(&launcher) {
                    if install == std::fs::canonicalize(folder).ok()? {
                        launch = Some(ConsoleLaunch::BloodborneLauncher { launcher: path_string(&launcher), emulator: path_string(&emulator), title_id: metadata.title_id.clone() });
                        break;
                    }
                }
            }
        }
        let emulator = ancestor.join("shadPS4.exe");
        if emulator.is_file() && launch.is_none() {
            launch = Some(ConsoleLaunch::ShadPs4 { emulator: path_string(&emulator), title_id: metadata.title_id.clone() });
            // Continue: a configured BBLauncher higher up should take priority.
        }
    }
    Some(Candidate { title: metadata.title, exe_path: path_string(eboot), folder: path_string(folder), score: 150, console_launch: Some(launch?) })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub fn sfo(title: &str, serial: &str, category: &str) -> Vec<u8> {
        let fields = [("TITLE", title), ("TITLE_ID", serial), ("CATEGORY", category)];
        let mut bytes = vec![0; 20 + fields.len() * 16];
        bytes[..4].copy_from_slice(b"\0PSF"); bytes[4..8].copy_from_slice(&0x101_u32.to_le_bytes());
        let keys = bytes.len(); let mut values = vec![];
        for (index, (key, value)) in fields.iter().enumerate() {
            let offset = 20 + index * 16;
            let key_offset = (bytes.len() - keys) as u16;
            bytes[offset..offset + 2].copy_from_slice(&key_offset.to_le_bytes());
            bytes[offset + 2..offset + 4].copy_from_slice(&0x204_u16.to_le_bytes());
            bytes[offset + 4..offset + 8].copy_from_slice(&(value.len() as u32 + 1).to_le_bytes());
            bytes[offset + 8..offset + 12].copy_from_slice(&(value.len() as u32 + 1).to_le_bytes());
            bytes[offset + 12..offset + 16].copy_from_slice(&(values.len() as u32).to_le_bytes());
            bytes.extend_from_slice(key.as_bytes()); bytes.push(0);
            values.extend_from_slice(value.as_bytes()); values.push(0);
        }
        let data = bytes.len(); bytes.extend_from_slice(&values);
        bytes[8..12].copy_from_slice(&(keys as u32).to_le_bytes()); bytes[12..16].copy_from_slice(&(data as u32).to_le_bytes());
        bytes[16..20].copy_from_slice(&(fields.len() as u32).to_le_bytes()); bytes
    }
    #[test] fn base_application_is_distinct_from_updates_dlc_and_saves() {
        let metadata = parse_sfo(&sfo("Bloodborne™", "CUSA03173", "gd")).unwrap();
        assert_eq!(metadata, Metadata { title: "Bloodborne".into(), title_id: "CUSA03173".into() });
        for category in ["gp", "ac", "sd"] { assert!(parse_sfo(&sfo("Bloodborne", "CUSA03173", category)).is_none()); }
    }
    #[test] fn malformed_metadata_is_rejected_without_panics() {
        let bytes = sfo("Bloodborne", "CUSA03173", "gd");
        for length in 0..bytes.len() { assert!(parse_sfo(&bytes[..length]).is_none()); }
        let mut corrupt = bytes.clone(); corrupt[8..12].copy_from_slice(&u32::MAX.to_le_bytes()); assert!(parse_sfo(&corrupt).is_none());
        corrupt = bytes.clone(); corrupt[16..20].copy_from_slice(&u32::MAX.to_le_bytes()); assert!(parse_sfo(&corrupt).is_none());
        assert!(parse_sfo(&sfo("", "CUSA03173", "gd")).is_none());
        assert!(parse_sfo(&sfo("Game", "../game", "gd")).is_none());
    }
}
