pub mod engine;
use engine::{Report, Settings};
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Default)]
struct Worker(Arc<AtomicBool>);
struct BusyGuard(Arc<AtomicBool>);
impl Drop for BusyGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

#[tauri::command]
async fn optimize_document(
    input: String,
    output: String,
    settings: Settings,
    worker: tauri::State<'_, Worker>,
) -> Result<Report, String> {
    if worker
        .0
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err("別のファイルを処理中です".into());
    }
    let guard = BusyGuard(worker.0.clone());
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        engine::optimize(Path::new(&input), Path::new(&output), &settings)
    })
    .await
    .map_err(|e| format!("処理スレッドが終了しました: {e}"))?
}

#[tauri::command]
async fn pdf_engine() -> Result<engine::pdf::PdfEngine, String> {
    tauri::async_runtime::spawn_blocking(engine::pdf::detect)
        .await
        .map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(Worker::default())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![optimize_document, pdf_engine])
        .run(tauri::generate_context!())
        .expect("Document Optimizer could not start");
}
