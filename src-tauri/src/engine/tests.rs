use super::*;
use std::io::{Cursor, Read, Write};
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

fn package(path: &Path, entries: &[(&str, &[u8])]) {
    let mut writer = ZipWriter::new(File::create(path).unwrap());
    for (name, data) in entries {
        writer
            .start_file(
                *name,
                SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
            )
            .unwrap();
        writer.write_all(data).unwrap();
    }
    writer.finish().unwrap();
}
fn entry(path: &Path, name: &str) -> Vec<u8> {
    let mut zip = ZipArchive::new(File::open(path).unwrap()).unwrap();
    let mut data = Vec::new();
    zip.by_name(name).unwrap().read_to_end(&mut data).unwrap();
    data
}
fn png() -> Vec<u8> {
    let image = image::RgbaImage::from_fn(128, 128, |x, y| {
        image::Rgba([x as u8, y as u8, 30, if x % 2 == 0 { 0 } else { 255 }])
    });
    let mut data = Vec::new();
    use image::ImageEncoder;
    image::codecs::png::PngEncoder::new_with_quality(
        &mut data,
        image::codecs::png::CompressionType::Fast,
        image::codecs::png::FilterType::NoFilter,
    )
    .write_image(&image, 128, 128, image::ExtendedColorType::Rgba8)
    .unwrap();
    data
}

#[test]
fn xml_bytes_are_never_rewritten() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("fixture.xlsx");
    let output = dir.path().join("result.xlsx");
    let xml = b"\xff\xfe<worksheet>image1.bmp</worksheet>";
    package(
        &input,
        &[
            ("xl/worksheets/sheet1.xml", xml),
            ("xl/media/image1.png", &png()),
        ],
    );
    optimize(&input, &output, &Settings::default()).unwrap();
    assert_eq!(entry(&output, "xl/worksheets/sheet1.xml"), xml);
}
#[test]
fn existing_output_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.xlsx");
    let output = dir.path().join("out.xlsx");
    package(&input, &[("document.xml", b"fixture")]);
    fs::write(&output, b"DO NOT OVERWRITE").unwrap();
    assert!(optimize(&input, &output, &Settings::default()).is_err());
    assert_eq!(fs::read(output).unwrap(), b"DO NOT OVERWRITE");
}
#[test]
fn larger_output_falls_back_to_exact_original() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.xlsx");
    let output = dir.path().join("out.xlsx");
    package(&input, &[("x", b"x")]);
    let report = optimize(&input, &output, &Settings::default()).unwrap();
    assert_eq!(fs::read(input).unwrap(), fs::read(output).unwrap());
    assert_eq!(report.saved, 0);
    assert!(report.unchanged);
}
#[test]
fn unsupported_image_formats_are_not_converted() {
    for ext in ["bmp", "gif", "tif", "tiff", "svg", "emf", "wmf", "webp"] {
        assert!(
            images::optimize(&format!("image.{ext}"), b"unchanged", &Settings::default()).is_err()
        );
    }
}
#[test]
fn jpeg_is_opt_in() {
    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 100)
        .encode_image(&image::DynamicImage::new_rgb8(16, 16))
        .unwrap();
    assert!(images::optimize("photo.jpg", &jpeg, &Settings::default()).is_err());
}
#[test]
fn png_preserves_every_rgba_pixel() {
    let original = png();
    let optimized = images::optimize("image.png", &original, &Settings::default()).unwrap();
    assert_eq!(
        image::load_from_memory(&original).unwrap().to_rgba8(),
        image::load_from_memory(&optimized).unwrap().to_rgba8()
    );
    assert!(optimized.len() < original.len());
}
#[test]
fn media_names_and_macro_bytes_do_not_collide() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.xlsm");
    let output = dir.path().join("out.xlsm");
    let data = png();
    let entries: Vec<(&str, &[u8])> = vec![
        ("xl/media/image1.png", &data),
        ("xl/media/image1.bmp", b"BMP"),
        ("xl/media/image11.gif", b"GIF89a"),
        ("xl/vbaProject.bin", b"synthetic VBA bytes"),
        (
            "xl/drawings/_rels/drawing1.xml.rels",
            b"../media/image1.bmp ../media/image11.gif",
        ),
        ("[Content_Types].xml", b"<Types/>"),
    ];
    package(&input, &entries);
    let before = fs::read(&input).unwrap();
    let report = optimize(&input, &output, &Settings::default()).unwrap();
    assert!(report.after < report.before);
    assert_eq!(report.optimized, 1);
    assert_eq!(report.skipped, 2);
    for (name, bytes) in &entries[1..] {
        assert_eq!(&entry(&output, name), bytes);
    }
    assert_eq!(fs::read(&input).unwrap(), before);
    assert_eq!(
        office::verify(File::open(input).unwrap()).unwrap(),
        office::verify(File::open(output).unwrap()).unwrap()
    );
}
#[test]
fn same_file_is_rejected_without_mutation() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.docx");
    package(&input, &[("a", b"x")]);
    let before = fs::read(&input).unwrap();
    assert!(optimize(&input, &input, &Settings::default()).is_err());
    assert_eq!(fs::read(input).unwrap(), before);
}
#[test]
fn extension_mismatch_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.docx");
    package(&input, &[("a", b"x")]);
    assert!(optimize(&input, &dir.path().join("out.pdf"), &Settings::default()).is_err());
}
#[test]
fn invalid_zip_leaves_no_partial_output_or_temp() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.pptx");
    let output = dir.path().join("out.pptx");
    fs::write(&input, b"not a ZIP").unwrap();
    assert!(optimize(&input, &output, &Settings::default()).is_err());
    assert!(!output.exists());
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}
#[test]
fn corrupt_crc_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.xlsx");
    package(&input, &[("x", b"unique-payload")]);
    let mut data = fs::read(&input).unwrap();
    let position = data
        .windows(14)
        .position(|s| s == b"unique-payload")
        .unwrap();
    data[position] = b'X';
    fs::write(&input, data).unwrap();
    assert!(optimize(&input, &dir.path().join("out.xlsx"), &Settings::default()).is_err());
}
#[test]
fn signed_office_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.xlsx");
    package(&input, &[("_xmlsignatures/sig1.xml", b"signature")]);
    assert!(
        optimize(&input, &dir.path().join("out.xlsx"), &Settings::default())
            .unwrap_err()
            .contains("電子署名")
    );
}
#[test]
fn apng_is_skipped_without_flattening() {
    let mut data = b"\x89PNG\r\n\x1a\n".to_vec();
    data.extend_from_slice(&0u32.to_be_bytes());
    data.extend_from_slice(b"acTL");
    data.extend_from_slice(&[0; 4]);
    assert!(
        images::optimize("animated.png", &data, &Settings::default())
            .unwrap_err()
            .contains("アニメーション")
    );
}
#[test]
fn malformed_png_is_skipped() {
    assert!(images::optimize("bad.png", b"bad", &Settings::default()).is_err());
}
#[test]
fn jpeg_lossy_retains_format_and_dimensions() {
    let mut original = Vec::new();
    let image = image::DynamicImage::new_rgb8(32, 24);
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut original, 100)
        .encode_image(&image)
        .unwrap();
    let settings = Settings {
        jpeg_lossy: true,
        ..Settings::default()
    };
    let output = images::optimize("photo.jpg", &original, &settings).unwrap();
    assert_eq!(
        image::guess_format(&output).unwrap(),
        image::ImageFormat::Jpeg
    );
    let decoder = image::ImageReader::new(Cursor::new(output))
        .with_guessed_format()
        .unwrap();
    assert_eq!(decoder.into_dimensions().unwrap(), (32, 24));
}
#[test]
fn pdf_requires_explicit_consent() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.pdf");
    fs::write(&input, b"%PDF-1.4\n%%EOF").unwrap();
    assert!(
        optimize(&input, &dir.path().join("out.pdf"), &Settings::default())
            .unwrap_err()
            .contains("同意")
    );
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}
#[test]
fn multi_picture_jpeg_is_not_flattened() {
    let mut original = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut original, 90)
        .encode_image(&image::DynamicImage::new_rgb8(16, 16))
        .unwrap();
    original.splice(2..2, [0xff, 0xe2, 0, 6, b'M', b'P', b'F', 0]);
    assert!(
        images::optimize(
            "multipicture.jpg",
            &original,
            &Settings {
                jpeg_lossy: true,
                ..Settings::default()
            }
        )
        .is_err()
    );
}
#[test]
#[ignore = "requires installed Ghostscript"]
fn real_ghostscript_optimizes_synthetic_pdf() {
    assert!(
        pdf::detect().available,
        "Install Ghostscript to run this integration test"
    );
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input.pdf");
    let output = dir.path().join("out.pdf");
    let content = format!("{}0 0 40 40 re f\n", " ".repeat(30_000));
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".into(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Resources << >> /Contents 4 0 R >>"
            .into(),
        format!(
            "<< /Length {} >>\nstream\n{}endstream",
            content.len(),
            content
        ),
    ];
    let mut pdf = "%PDF-1.4\n".to_string();
    let mut offsets = Vec::new();
    for (i, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.push_str(&format!("{} 0 obj\n{}\nendobj\n", i + 1, object));
    }
    let start = pdf.len();
    pdf.push_str("xref\n0 5\n0000000000 65535 f \n");
    for offset in offsets {
        pdf.push_str(&format!("{offset:010} 00000 n \n"));
    }
    pdf.push_str(&format!(
        "trailer\n<< /Size 5 /Root 1 0 R >>\nstartxref\n{start}\n%%EOF\n"
    ));
    fs::write(&input, &pdf).unwrap();
    let report = optimize(
        &input,
        &output,
        &Settings {
            pdf_acknowledged: true,
            ..Settings::default()
        },
    )
    .unwrap();
    assert!(report.saved > 0);
    assert!(fs::read(&output).unwrap().starts_with(b"%PDF-"));
    assert_eq!(fs::read(&input).unwrap(), pdf.as_bytes());
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
    println!(
        "PDF integration: {} -> {} bytes; {}",
        report.before,
        report.after,
        pdf::detect().version.unwrap()
    );
}
#[test]
#[ignore = "requires installed Ghostscript"]
fn real_ghostscript_error_keeps_diagnostics_and_cleans_temp() {
    assert!(pdf::detect().available);
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("bad.pdf");
    let output = dir.path().join("out.pdf");
    fs::write(&input, b"%PDF-1.4\ninvalid document\n%%EOF").unwrap();
    let error = optimize(
        &input,
        &output,
        &Settings {
            pdf_acknowledged: true,
            ..Settings::default()
        },
    )
    .unwrap_err();
    assert!(error.contains("Ghostscript終了コード"));
    assert!(!output.exists());
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    println!("Expected external failure: {error}");
}
#[test]
fn duplicate_zip_names_are_rejected_before_library_deduplication() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.xlsx");
    package(&input, &[("a", b"one"), ("b", b"two")]);
    let mut zip = ZipArchive::new(File::open(&input).unwrap()).unwrap();
    let second = zip.by_name("b").unwrap();
    let local = second.header_start() as usize + 30;
    let central = second.central_header_start() as usize + 46;
    drop(second);
    drop(zip);
    let mut bytes = fs::read(&input).unwrap();
    bytes[local] = b'a';
    bytes[central] = b'a';
    fs::write(&input, bytes).unwrap();
    assert!(optimize(&input, &dir.path().join("out.xlsx"), &Settings::default()).is_err());
}
#[test]
fn non_pdf_payload_is_rejected_before_invoking_external_engine() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("fake.pdf");
    fs::write(&input, b"%!PS\nshowpage").unwrap();
    let error = optimize(
        &input,
        &dir.path().join("out.pdf"),
        &Settings {
            pdf_acknowledged: true,
            ..Settings::default()
        },
    )
    .unwrap_err();
    assert!(error.contains("PDFヘッダー"));
}
#[test]
fn invalid_settings_are_rejected() {
    let settings = Settings {
        jpeg_quality: 0,
        ..Settings::default()
    };
    assert!(
        optimize(Path::new("missing.xlsx"), Path::new("out.xlsx"), &settings)
            .unwrap_err()
            .contains("50")
    );
}
