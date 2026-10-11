//! Bounded readers for the installation-only parts of binary launcher records.
use std::{io::Read, path::Path};
use sha1::{Digest, Sha1};

pub fn read(path: &Path, limit: usize) -> Option<Vec<u8>> {
    let file = std::fs::File::open(path).ok()?;
    if file.metadata().ok()?.len() > limit as u64 { return None; }
    let mut bytes = vec![];
    file.take(limit as u64 + 1).read_to_end(&mut bytes).ok()?;
    (bytes.len() <= limit).then_some(bytes)
}
fn u32_at(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(offset..offset + 4)?.try_into().ok()?))
}
fn fstring(bytes: &[u8], offset: &mut usize) -> Option<String> {
    let count = u32_at(bytes, *offset)? as i32; *offset += 4;
    if count == 0 { return Some(String::new()); }
    let length = count.unsigned_abs() as usize;
    if length > 16384 { return None; }
    if count > 0 {
        let value = bytes.get(*offset..*offset + length)?; *offset += length;
        if value.last() != Some(&0) { return None; }
        String::from_utf8(value[..length - 1].to_vec()).ok()
    } else {
        let value = bytes.get(*offset..*offset + length * 2)?; *offset += length * 2;
        let words: Vec<u16> = value.chunks_exact(2).map(|pair| u16::from_le_bytes([pair[0], pair[1]])).collect();
        if words.last() != Some(&0) { return None; }
        String::from_utf16(&words[..length - 1]).ok()
    }
}
/// Epic BuildPatch manifest metadata. SHA-1 verifies the recorded payload, not publisher trust.
pub fn epic_manifest(bytes: &[u8]) -> Option<(String, String)> {
    if u32_at(bytes, 0)? != 0x44BEC00C { return None; }
    let header = u32_at(bytes, 4)? as usize;
    let unpacked = u32_at(bytes, 8)? as usize;
    let packed = u32_at(bytes, 12)? as usize;
    if !(41..=4096).contains(&header) || unpacked > 64 * 1024 * 1024 || packed > 16 * 1024 * 1024 { return None; }
    let flags = *bytes.get(36)?;
    if flags & !1 != 0 { return None; } // Encrypted/new storage formats require the official client.
    if bytes.len() != header.checked_add(packed)? { return None; }
    let payload = bytes.get(header..)?;
    let body = if flags == 1 {
        let mut body = Vec::new();
        flate2::read::ZlibDecoder::new(payload).take(unpacked as u64 + 1).read_to_end(&mut body).ok()?;
        body
    } else { payload.to_vec() };
    if body.len() != unpacked || Sha1::digest(&body).as_slice() != bytes.get(16..36)? { return None; }
    let meta_size = u32_at(&body, 0)? as usize;
    let meta = body.get(..meta_size)?;
    let mut offset = 14;
    let app = fstring(meta, &mut offset)?;
    let _build = fstring(meta, &mut offset)?;
    let launch = fstring(meta, &mut offset)?;
    Some((app, launch))
}

fn varint(bytes: &mut &[u8]) -> Option<u64> {
    let mut value = 0;
    for shift in (0..70).step_by(7) {
        let byte = *bytes.first()?; *bytes = &bytes[1..];
        if shift == 63 && byte > 1 { return None; }
        value |= ((byte & 127) as u64) << shift;
        if byte & 128 == 0 { return Some(value); }
    }
    None
}
fn fields(mut bytes: &[u8]) -> Option<Vec<(u64, &[u8])>> {
    let mut result = vec![];
    while !bytes.is_empty() {
        if result.len() >= 16384 { return None; }
        let tag = varint(&mut bytes)?;
        if tag >> 3 == 0 { return None; }
        match tag & 7 {
            0 => { varint(&mut bytes)?; },
            1 => { bytes = bytes.get(8..)?; },
            2 => {
                let size = usize::try_from(varint(&mut bytes)?).ok()?;
                let value = bytes.get(..size)?; bytes = bytes.get(size..)?;
                result.push((tag >> 3, value));
            },
            5 => { bytes = bytes.get(4..)?; },
            _ => return None,
        }
    }
    Some(result)
}
fn field<'a>(fields: &[(u64, &'a [u8])], id: u64) -> Option<&'a [u8]> {
    let mut matches = fields.iter().filter(|(key, _)| *key == id);
    let value = matches.next()?.1;
    if matches.next().is_some() { return None; }
    Some(value)
}
/// Only internal product IDs and install paths; account/authentication fields are ignored.
pub fn battle_net(bytes: &[u8]) -> Option<Vec<(String, String)>> {
    if bytes.len() > 4 * 1024 * 1024 { return None; }
    let mut products = vec![];
    for (_, value) in fields(bytes)?.into_iter().filter(|(id, _)| *id == 1) {
        let product = fields(value)?;
        let Some(internal) = field(&product, 1) else { continue; };
        let Some(data) = field(&product, 3) else { continue; };
        let data = fields(data)?;
        let Some(path) = field(&data, 1) else { continue; };
        if internal.len() > 200 || path.len() > 32768 { continue; }
        products.push((std::str::from_utf8(internal).ok()?.into(), std::str::from_utf8(path).ok()?.into()));
    }
    Some(products)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn string(value: &str, bytes: &mut Vec<u8>) {
        bytes.extend(((value.len() + 1) as u32).to_le_bytes()); bytes.extend(value.as_bytes()); bytes.push(0);
    }
    fn manifest(compressed: bool) -> Vec<u8> {
        let mut body = vec![0; 14];
        string("FortniteReleaseBuilds", &mut body); string("build", &mut body);
        string("FortniteGame/Binaries/Win64/FortniteBootstrapper.exe", &mut body);
        let length = body.len() as u32; body[..4].copy_from_slice(&length.to_le_bytes());
        let payload = if compressed {
            use std::io::Write;
            let mut encoder = flate2::write::ZlibEncoder::new(vec![], flate2::Compression::default());
            encoder.write_all(&body).unwrap(); encoder.finish().unwrap()
        } else { body.clone() };
        let mut bytes = vec![];
        for number in [0x44BEC00C, 41, length, payload.len() as u32] { bytes.extend(number.to_le_bytes()); }
        bytes.extend(Sha1::digest(&body)); bytes.push(u8::from(compressed)); bytes.extend(21_u32.to_le_bytes()); bytes.extend(payload); bytes
    }
    #[test] fn epic_metadata_checks_compression_lengths_and_payload_integrity() {
        for compressed in [false, true] {
            let bytes = manifest(compressed);
            assert_eq!(epic_manifest(&bytes).unwrap().0, "FortniteReleaseBuilds");
            let mut corrupt = bytes.clone(); corrupt[16] ^= 1; assert!(epic_manifest(&corrupt).is_none());
            let mut encrypted = bytes.clone(); encrypted[36] |= 2; assert!(epic_manifest(&encrypted).is_none());
            let mut oversized = bytes.clone(); oversized[8..12].copy_from_slice(&u32::MAX.to_le_bytes()); assert!(epic_manifest(&oversized).is_none());
            assert!(epic_manifest(&bytes[..bytes.len() - 1]).is_none());
        }
    }
    #[test] fn unreal_strings_are_bounded_and_decode_utf16() {
        let mut bytes = (-3_i32).to_le_bytes().to_vec();
        for word in [0x0130_u16, 0x011f, 0] { bytes.extend(word.to_le_bytes()); }
        assert_eq!(fstring(&bytes, &mut 0).as_deref(), Some("İğ"));
        assert!(fstring(&i32::MIN.to_le_bytes(), &mut 0).is_none());
    }
    #[test] fn battle_net_reads_installation_fields_and_rejects_broken_protobuf() {
        // InstalledProductInfo[] -> InternalId, ignored ProductId, InstallData.Path.
        let bytes = b"\x0a\x19\x0a\x03wow\x12\x04guid\x1a\x0c\x0a\x0aD:/WoW/bin";
        assert_eq!(battle_net(bytes).unwrap(), [("wow".into(), "D:/WoW/bin".into())]);
        for bytes in [b"\x0a\xff".as_slice(), b"\x00", b"\x0b", b"\x0a\x04abc"] { assert!(battle_net(bytes).is_none()); }
    }
}
