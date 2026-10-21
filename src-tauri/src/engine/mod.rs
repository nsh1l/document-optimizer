//! Native engine. All writes go to an adjacent private staging file and publish
//! with no-clobber semantics. The original remains open read-only throughout.
mod images;
mod office;
pub mod pdf;
#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Seek},
    path::{Path, PathBuf},
};

#[derive(Clone, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    pub jpeg_lossy: bool,
    pub jpeg_quality: u8,
    pub pdf_quality: String,
    pub pdf_acknowledged: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            jpeg_lossy: false,
            jpeg_quality: 85,
            pdf_quality: "ebook".into(),
            pdf_acknowledged: false,
        }
    }
}

#[derive(Default, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub output: String,
    pub before: u64,
    pub after: u64,
    pub saved: u64,
    pub optimized: usize,
    pub skipped: usize,
    pub unchanged: bool,
    pub messages: Vec<String>,
    pub omitted_messages: usize,
}
impl Report {
    fn note(&mut self, message: String) {
        if self.messages.len() < 100 {
            self.messages.push(message);
        } else {
            self.omitted_messages += 1;
        }
    }
}

fn extension(path: &Path) -> String {
    path.extension()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

fn destination(input: &Path, output: &Path) -> Result<PathBuf, String> {
    if input == output {
        return Err("入力と出力に同じファイルは指定できません".into());
    }
    if fs::symlink_metadata(output).is_ok() {
        return Err("出力先は既に存在します。別名を選んでください".into());
    }
    if extension(input) != extension(output) {
        return Err("入力と出力の拡張子を一致させてください".into());
    }
    let name = output.file_name().ok_or("出力ファイル名が必要です")?;
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .canonicalize()
        .map_err(|e| format!("保存先フォルダーを開けません: {e}"))?;
    if !parent.is_dir() {
        return Err("保存先はフォルダーではありません".into());
    }
    Ok(parent.join(name))
}

fn open_source(path: &Path) -> Result<File, String> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(1); // FILE_SHARE_READ: deny modification/deletion during work.
    }
    options.open(path).map_err(|e| {
        format!("入力を開けません（他のアプリで編集している場合は閉じてください）: {e}")
    })
}

pub fn optimize(input: &Path, output: &Path, settings: &Settings) -> Result<Report, String> {
    if !(50..=95).contains(&settings.jpeg_quality) {
        return Err("JPEG品質は50〜95で指定してください".into());
    }
    if !["screen", "ebook", "prepress"].contains(&settings.pdf_quality.as_str()) {
        return Err("PDF品質の指定が不正です".into());
    }
    let input = input
        .canonicalize()
        .map_err(|e| format!("入力が見つかりません: {e}"))?;
    let kind = extension(&input);
    if !["xlsx", "xlsm", "docx", "pptx", "pdf"].contains(&kind.as_str()) {
        return Err("対応形式は XLSX / XLSM / DOCX / PPTX / PDF です".into());
    }
    let output = destination(&input, output)?;
    let mut source = open_source(&input)?;
    let metadata = source.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() > 1024 * 1024 * 1024 {
        return Err("通常ファイル（1 GiB以下）を選んでください".into());
    }
    let parent = output.parent().ok_or("保存先が不正です")?;
    let mut staging = tempfile::Builder::new()
        .prefix(".document-optimizer-")
        .tempfile_in(parent)
        .map_err(|e| e.to_string())?;
    let mut report = Report {
        output: output.to_string_lossy().into(),
        before: metadata.len(),
        ..Report::default()
    };
    if kind == "pdf" {
        pdf::rewrite(
            &mut source,
            staging.as_file_mut(),
            parent,
            settings,
            &mut report,
        )?;
    } else {
        office::rewrite(&mut source, staging.as_file_mut(), settings, &mut report)?;
    }
    report.after = staging
        .as_file()
        .metadata()
        .map_err(|e| e.to_string())?
        .len();
    if report.after >= report.before {
        staging
            .as_file_mut()
            .set_len(0)
            .map_err(|e| e.to_string())?;
        staging.as_file_mut().rewind().map_err(|e| e.to_string())?;
        source.rewind().map_err(|e| e.to_string())?;
        io::copy(&mut source, staging.as_file_mut()).map_err(|e| e.to_string())?;
        report.after = report.before;
        report.unchanged = true;
        report.skipped += report.optimized;
        report.optimized = 0;
        report.note("削減できないため、元ファイルと同じ内容を別名で保存しました".into());
    }
    report.saved = report.before - report.after;
    staging.as_file().sync_all().map_err(|e| e.to_string())?;
    // A destination created after validation must not be overwritten either.
    staging.persist_noclobber(&output).map_err(|e| {
        format!(
            "保存できません（既存ファイルは上書きしません）: {}",
            e.error
        )
    })?;
    Ok(report)
}
