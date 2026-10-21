//! ZIP package handling never decodes XML or changes entry names.
use super::{Report, Settings, images};
use std::{
    collections::HashSet,
    fs::File,
    io::{self, Read, Seek, Write},
};
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

const MAX_ENTRY: u64 = 256 * 1024 * 1024;
const MAX_TOTAL: u64 = 2 * 1024 * 1024 * 1024;
const MAX_IMAGE: u64 = 32 * 1024 * 1024;

fn media(name: &str) -> bool {
    ["xl/media/", "word/media/", "ppt/media/"]
        .iter()
        .any(|p| name.starts_with(p))
        && !name.ends_with('/')
}

/// Reading every entry to EOF validates its CRC, including entries copied raw later.
pub fn verify<R: Read + Seek>(reader: R) -> Result<Vec<String>, String> {
    let archive = ZipArchive::new(reader).map_err(|e| format!("ZIPを開けません: {e}"))?;
    let expected_count = archive.len();
    let directory_start = archive.central_directory_start();
    let mut reader = archive.into_inner();
    reader
        .seek(io::SeekFrom::Start(directory_start))
        .map_err(|e| e.to_string())?;
    // zip 6 indexes by name and silently coalesces duplicate central entries.
    // Count the on-disk central records before accepting that indexed view.
    let mut records = 0;
    loop {
        let mut signature = [0; 4];
        reader
            .read_exact(&mut signature)
            .map_err(|e| e.to_string())?;
        if signature != *b"PK\x01\x02" {
            break;
        }
        let mut header = [0; 42];
        reader.read_exact(&mut header).map_err(|e| e.to_string())?;
        let lengths: u64 = [24, 26, 28]
            .into_iter()
            .map(|p| u16::from_le_bytes([header[p], header[p + 1]]) as u64)
            .sum();
        reader
            .seek(io::SeekFrom::Current(lengths as i64))
            .map_err(|e| e.to_string())?;
        records += 1;
        if records > 50_000 {
            return Err("ZIP項目数の上限です (50,000)".into());
        }
    }
    if records != expected_count {
        return Err("重複または不整合のあるZIP項目は処理しません".into());
    }
    reader.rewind().map_err(|e| e.to_string())?;
    let mut archive = ZipArchive::new(reader).map_err(|e| e.to_string())?;
    if archive.len() > 50_000 {
        return Err("ZIP項目数の上限です (50,000)".into());
    }
    let mut names = Vec::new();
    let mut unique = HashSet::new();
    let mut total = 0u64;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.name().to_owned();
        if !unique.insert(name.clone()) {
            return Err(format!("重複したZIP名: {name}"));
        }
        if name.to_ascii_lowercase().starts_with("_xmlsignatures/") {
            return Err("電子署名付きOffice文書は処理しません".into());
        }
        if entry.size() > MAX_ENTRY {
            return Err(format!("ZIP項目の展開上限 (256 MiB): {name}"));
        }
        total = total
            .checked_add(entry.size())
            .ok_or("ZIPサイズが不正です")?;
        if total > MAX_TOTAL {
            return Err("ZIP展開合計の上限です (2 GiB)".into());
        }
        let size = entry.size();
        let copied = io::copy(&mut entry.by_ref().take(MAX_ENTRY + 1), &mut io::sink())
            .map_err(|e| format!("ZIP検証失敗 ({name}): {e}"))?;
        if copied != size {
            return Err(format!("ZIPサイズ不一致: {name}"));
        }
        names.push(name);
    }
    Ok(names)
}

pub fn rewrite(
    input: &mut File,
    output: &mut File,
    settings: &Settings,
    report: &mut Report,
) -> Result<(), String> {
    input.rewind().map_err(|e| e.to_string())?;
    let expected = verify(&mut *input)?;
    input.rewind().map_err(|e| e.to_string())?;
    let mut archive = ZipArchive::new(&mut *input).map_err(|e| e.to_string())?;
    let mut writer = ZipWriter::new(&mut *output);
    writer.set_raw_comment(archive.comment().into());
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.name().to_owned();
        if media(&name) {
            let candidate = if entry.size() > MAX_IMAGE {
                Err("画像サイズ上限 (32 MiB) のため保持します".into())
            } else {
                let mut data = Vec::with_capacity(entry.size() as usize);
                entry.read_to_end(&mut data).map_err(|e| e.to_string())?;
                images::optimize(&name, &data, settings).and_then(|encoded| {
                    if encoded.len() < data.len() {
                        Ok(encoded)
                    } else {
                        Err("画像のサイズ削減なし".into())
                    }
                })
            };
            match candidate {
                Ok(data) => {
                    let mut options = SimpleFileOptions::default()
                        .compression_method(CompressionMethod::Deflated);
                    if let Some(time) = entry.last_modified() {
                        options = options.last_modified_time(time);
                    }
                    if let Some(mode) = entry.unix_mode() {
                        options = options.unix_permissions(mode);
                    }
                    writer
                        .start_file(&name, options)
                        .map_err(|e| e.to_string())?;
                    writer.write_all(&data).map_err(|e| e.to_string())?;
                    report.optimized += 1;
                    continue;
                }
                Err(reason) => {
                    report.skipped += 1;
                    report.note(format!("{name}: {reason}"));
                }
            }
        }
        // Re-open because decoding an image advanced its stream. The ZIP crate copies
        // the compressed payload, preserving untouched parts without recompression.
        drop(entry);
        writer
            .raw_copy_file(archive.by_index(i).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    }
    writer.finish().map_err(|e| e.to_string())?;
    output.rewind().map_err(|e| e.to_string())?;
    if verify(&mut *output)? != expected {
        return Err("出力ZIPの項目が一致しません".into());
    }
    Ok(())
}
