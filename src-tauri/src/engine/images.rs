//! Format-preserving transformations. A skipped image is always copied verbatim.
use super::Settings;
use image::{ImageDecoder, ImageFormat, ImageReader};
use std::{io::Cursor, time::Duration};

pub fn optimize(name: &str, data: &[u8], settings: &Settings) -> Result<Vec<u8>, String> {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".png") {
        if !data.starts_with(b"\x89PNG\r\n\x1a\n") {
            return Err("PNG形式が一致しません".into());
        }
        // Inspect chunk boundaries, not arbitrary byte substrings (IDAT may contain acTL).
        let mut offset = 8usize;
        while offset + 12 <= data.len() {
            let size = u32::from_be_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
            let end = offset
                .checked_add(size)
                .and_then(|n| n.checked_add(12))
                .ok_or("PNGチャンクが不正です")?;
            if end > data.len() {
                return Err("PNGチャンクが不正です".into());
            }
            if &data[offset + 4..offset + 8] == b"acTL" {
                return Err("アニメーションPNGは保持します".into());
            }
            offset = end;
        }
        let mut options = oxipng::Options::from_preset(2);
        options.strip = oxipng::StripChunks::None;
        options.optimize_alpha = false;
        options.scale_16 = false;
        options.interlace = None;
        options.timeout = Some(Duration::from_secs(20));
        options.max_decompressed_size = Some(128 * 1024 * 1024);
        oxipng::optimize_from_memory(data, &options).map_err(|e| format!("PNGを保持: {e}"))
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        if !settings.jpeg_lossy {
            return Err("JPEG再圧縮は無効です".into());
        }
        if !data.starts_with(&[0xff, 0xd8]) {
            return Err("JPEG形式が一致しません".into());
        }
        let mut offset = 2;
        while offset < data.len() {
            if data[offset] != 0xff {
                return Err("JPEGマーカーが不正です".into());
            }
            while offset < data.len() && data[offset] == 0xff {
                offset += 1;
            }
            let marker = *data.get(offset).ok_or("JPEGが途中で切れています")?;
            offset += 1;
            if marker == 0xda || marker == 0xd9 {
                break;
            }
            if (0xe1..=0xef).contains(&marker) || marker == 0xfe {
                return Err("追加情報・複数画像を含むJPEGは保持します".into());
            }
            if (0xd0..=0xd8).contains(&marker) || marker == 0x01 {
                continue;
            }
            let length = data
                .get(offset..offset + 2)
                .ok_or("JPEGが途中で切れています")?;
            let length = u16::from_be_bytes([length[0], length[1]]) as usize;
            if length < 2 || offset + length > data.len() {
                return Err("JPEGのセグメント長が不正です".into());
            }
            offset += length;
        }
        let mut reader = ImageReader::with_format(Cursor::new(data), ImageFormat::Jpeg);
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(20_000);
        limits.max_image_height = Some(20_000);
        limits.max_alloc = Some(128 * 1024 * 1024);
        reader.limits(limits);
        let mut decoder = reader.into_decoder().map_err(|e| e.to_string())?;
        // Re-encoding would discard color/orientation metadata. Keep those JPEGs intact.
        if decoder.icc_profile().map_err(|e| e.to_string())?.is_some()
            || decoder
                .exif_metadata()
                .map_err(|e| e.to_string())?
                .is_some()
            || decoder.xmp_metadata().map_err(|e| e.to_string())?.is_some()
        {
            return Err("色・撮影情報付きJPEGは保持します".into());
        }
        if decoder.total_bytes() > 128 * 1024 * 1024 {
            return Err("画像の展開サイズ上限です".into());
        }
        let image = image::DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())?;
        let mut output = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut output, settings.jpeg_quality)
            .encode_image(&image.to_rgb8())
            .map_err(|e| e.to_string())?;
        Ok(output)
    } else {
        Err("対象外の画像形式は保持します".into())
    }
}
