//! Ghostscript is an external, optional dependency. Nothing is downloaded.
use super::{Report, Settings};
use serde::Serialize;
use std::{
    env,
    fs::{self, File},
    io::{self, Read, Seek},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

#[derive(Serialize)]
pub struct PdfEngine {
    pub available: bool,
    pub path: Option<String>,
    pub version: Option<String>,
}

fn command(path: &Path) -> Command {
    let mut cmd = Command::new(path);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    cmd
}

fn candidates() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(path) = env::var_os("PATH") {
        for dir in env::split_paths(&path).filter(|p| p.is_absolute()) {
            paths.push(dir.join("gswin64c.exe"));
        }
    }
    for root in [r"C:\Program Files\gs", r"C:\Program Files (x86)\gs"] {
        if let Ok(entries) = fs::read_dir(root) {
            let mut dirs: Vec<_> = entries.flatten().map(|e| e.path()).collect();
            dirs.sort();
            dirs.reverse();
            paths.extend(dirs.into_iter().map(|p| p.join("bin/gswin64c.exe")));
        }
    }
    paths
}

// Redirect diagnostics to a file, bound runtime and log growth, always reap a child.
fn execute(exe: &Path, dir: &Path, args: &[&str], timeout: Duration) -> Result<String, String> {
    let log = tempfile::tempfile_in(dir).map_err(|e| e.to_string())?;
    let mut child = command(exe)
        .current_dir(dir)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log.try_clone().map_err(|e| e.to_string())?))
        .stderr(Stdio::from(log.try_clone().map_err(|e| e.to_string())?))
        .spawn()
        .map_err(|e| format!("Ghostscriptを起動できません: {e}"))?;
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(e.to_string());
            }
        }
        if started.elapsed() > timeout
            || log
                .metadata()
                .map(|m| m.len() > 4 * 1024 * 1024)
                .unwrap_or(true)
        {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Ghostscriptの時間・ログ上限に達しました。出力は保存していません".into());
        }
        thread::sleep(Duration::from_millis(80));
    };
    let mut log = log;
    log.rewind().map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    log.take(16 * 1024)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&bytes).trim().to_string();
    if !status.success() {
        return Err(format!("Ghostscript終了コード {:?}: {text}", status.code()));
    }
    Ok(text)
}

pub fn detect() -> PdfEngine {
    for path in candidates().into_iter().filter(|p| p.is_file()) {
        if let Ok(dir) = tempfile::tempdir()
            && let Ok(version) = execute(&path, dir.path(), &["--version"], Duration::from_secs(5))
            && version.chars().next().is_some_and(|c| c.is_ascii_digit())
        {
            return PdfEngine {
                available: true,
                path: Some(path.to_string_lossy().into()),
                version: Some(version),
            };
        }
    }
    PdfEngine {
        available: false,
        path: None,
        version: None,
    }
}

pub fn rewrite(
    input: &mut File,
    output: &mut File,
    parent: &Path,
    settings: &Settings,
    report: &mut Report,
) -> Result<(), String> {
    if !settings.pdf_acknowledged {
        return Err("PDF再生成の注意事項への同意が必要です".into());
    }
    // Ghostscript also accepts PostScript: reject disguised input before spawning it.
    input.rewind().map_err(|e| e.to_string())?;
    let mut header = [0; 5];
    input
        .read_exact(&mut header)
        .map_err(|_| "PDFヘッダーを読み取れません".to_string())?;
    if &header != b"%PDF-" {
        return Err("PDFヘッダーがありません".into());
    }
    let detected = detect();
    let exe = detected.path.ok_or("PDFには別途Ghostscriptが必要です。64-bit版のgswin64c.exeをPATHまたはC:\\Program Files\\gsに配置してください")?;
    let dir = tempfile::Builder::new()
        .prefix("document-pdf-")
        .tempdir_in(parent)
        .map_err(|e| e.to_string())?;
    input.rewind().map_err(|e| e.to_string())?;
    io::copy(
        input,
        &mut File::create(dir.path().join("input.pdf")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let setting = format!("-dPDFSETTINGS=/{}", settings.pdf_quality);
    execute(
        Path::new(&exe),
        dir.path(),
        &[
            "-dSAFER",
            "-dBATCH",
            "-dNOPAUSE",
            "-dPDFSTOPONERROR",
            "-sDEVICE=pdfwrite",
            &setting,
            "-sOutputFile=candidate.pdf",
            "-f",
            "input.pdf",
        ],
        Duration::from_secs(300),
    )?;
    let mut candidate = File::open(dir.path().join("candidate.pdf")).map_err(|e| e.to_string())?;
    let mut header = [0; 5];
    candidate
        .read_exact(&mut header)
        .map_err(|e| e.to_string())?;
    if &header != b"%PDF-" {
        return Err("出力がPDFではありません".into());
    }
    execute(
        Path::new(&exe),
        dir.path(),
        &[
            "-dSAFER",
            "-dBATCH",
            "-dNOPAUSE",
            "-dPDFSTOPONERROR",
            "-sDEVICE=nullpage",
            "-f",
            "candidate.pdf",
        ],
        Duration::from_secs(300),
    )?;
    candidate.rewind().map_err(|e| e.to_string())?;
    io::copy(&mut candidate, output).map_err(|e| e.to_string())?;
    report.note("PDFをGhostscriptで再生成・読込検証しました。署名・フォーム・注釈・色・アクセシビリティの保持は保証しません。目視で確認してください".into());
    Ok(())
}
