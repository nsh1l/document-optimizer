import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open, save } from "@tauri-apps/plugin-dialog";
import { defaultSettings } from "./types";
import type { Job, PdfEngine, Report } from "./types";
import { filterSupportedPaths, runQueue, suggestedOutput, supportedExtensions } from "./queue";
import { formatBytes, JobRow } from "./components/JobRow";
import { SettingsPanel } from "./components/SettingsPanel";
import "./App.css";

function initialTheme() {
  try {
    return localStorage.getItem("optimizer-theme") === "dark" ? "dark" : "light";
  } catch {
    return "light";
  }
}

export default function App() {
  const [jobs, setJobs] = useState<Job[]>([]);
  const [settings, setSettings] = useState(defaultSettings);
  const [directory, setDirectory] = useState("");
  const [engine, setEngine] = useState<PdfEngine | null>(null);
  const [busy, setBusy] = useState(false);
  const [stopping, setStopping] = useState(false);
  const [dropActive, setDropActive] = useState(false);
  const [notice, setNotice] = useState("ファイルを追加して、保存先を確認してください。");
  const [theme, setTheme] = useState(initialTheme);
  const active = useRef(false);
  const stop = useRef(false);
  const knownInputs = useRef(new Set<string>());

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    try {
      localStorage.setItem("optimizer-theme", theme);
    } catch {
      // The selected theme remains active for this session.
    }
  }, [theme]);

  const addPaths = useCallback((paths: string[]) => {
    if (active.current) {
      setNotice("処理中はファイルを追加できません。終了後に追加してください。");
      return;
    }

    const { accepted, unsupported } = filterSupportedPaths(paths);
    const additions: string[] = [];
    let duplicates = 0;
    for (const input of accepted) {
      const key = input.toLocaleLowerCase();
      if (knownInputs.current.has(key)) {
        duplicates += 1;
        continue;
      }
      knownInputs.current.add(key);
      additions.push(input);
    }

    if (additions.length > 0) {
      setJobs(current => [
        ...current,
        ...additions.map(input => ({
          id: crypto.randomUUID(),
          input,
          output: suggestedOutput(input, directory),
          status: "pending" as const,
        })),
      ]);
    }

    if (additions.length === 0 && unsupported.length === 0 && duplicates === 0) {
      setNotice("ファイルがありませんでした。");
    } else {
      const details = [
        additions.length > 0 ? `${additions.length}件を追加` : "追加できるファイルはありません",
        unsupported.length > 0 ? `対応外 ${unsupported.length}件` : "",
        duplicates > 0 ? `重複 ${duplicates}件` : "",
      ].filter(Boolean);
      setNotice(`${details.join("・")}。`);
    }
  }, [directory]);

  useEffect(() => {
    let disposed = false;
    let unlistenClose: (() => void) | undefined;
    let unlistenDrop: (() => void) | undefined;

    invoke<PdfEngine>("pdf_engine")
      .then(value => {
        if (!disposed) setEngine(value);
      })
      .catch(error => {
        if (!disposed) {
          setEngine({ available: false, path: null, version: null });
          setNotice(`PDFエンジンの確認に失敗: ${String(error)}`);
        }
      });

    getCurrentWindow()
      .onCloseRequested(event => {
        if (active.current) {
          event.preventDefault();
          setNotice("処理中は閉じられません。停止を予約して、終了をお待ちください。");
        }
      })
      .then(unlisten => {
        if (disposed) unlisten();
        else unlistenClose = unlisten;
      })
      .catch(error => {
        if (!disposed) setNotice(`ウィンドウ監視を開始できません: ${String(error)}`);
      });

    getCurrentWebview()
      .onDragDropEvent(event => {
        if (event.payload.type === "enter" || event.payload.type === "over") {
          setDropActive(true);
        } else if (event.payload.type === "drop") {
          setDropActive(false);
          addPaths(event.payload.paths);
        } else {
          setDropActive(false);
        }
      })
      .then(unlisten => {
        if (disposed) unlisten();
        else unlistenDrop = unlisten;
      })
      .catch(error => {
        if (!disposed) setNotice(`ドラッグ＆ドロップを有効にできません: ${String(error)}`);
      });

    return () => {
      disposed = true;
      unlistenClose?.();
      unlistenDrop?.();
    };
  }, [addPaths]);

  function applyDirectory(value: string) {
    setDirectory(value);
    setJobs(current => current.map(job =>
      ["pending", "error", "cancelled"].includes(job.status)
        ? { ...job, output: suggestedOutput(job.input, value) }
        : job,
    ));
  }

  async function addFiles() {
    try {
      const selected = await open({
        multiple: true,
        filters: [{ name: "Office / PDF", extensions: [...supportedExtensions] }],
      });
      if (selected) addPaths(Array.isArray(selected) ? selected : [selected]);
    } catch (error) {
      setNotice(`ファイル選択に失敗: ${String(error)}`);
    }
  }

  async function chooseDirectory() {
    try {
      const value = await open({ directory: true, multiple: false });
      if (typeof value === "string") applyDirectory(value);
    } catch (error) {
      setNotice(`保存先選択に失敗: ${String(error)}`);
    }
  }

  async function chooseOutput(job: Job) {
    try {
      const extension = job.input.split(".").pop()!.toLowerCase();
      const value = await save({
        defaultPath: job.output,
        filters: [{ name: extension.toUpperCase(), extensions: [extension] }],
      });
      if (value) update(job.id, { output: value });
    } catch (error) {
      setNotice(`保存先選択に失敗: ${String(error)}`);
    }
  }

  function update(id: string, patch: Partial<Job>) {
    setJobs(current => current.map(job => job.id === id ? { ...job, ...patch } : job));
  }

  function removeJob(job: Job) {
    knownInputs.current.delete(job.input.toLocaleLowerCase());
    setJobs(current => current.filter(item => item.id !== job.id));
  }

  async function start() {
    if (active.current) return;

    active.current = true;
    stop.current = false;
    setBusy(true);
    setStopping(false);
    setNotice("順番に処理しています。ファイル内の進捗率は表示しません。");

    try {
      await runQueue(
        jobs,
        job => invoke<Report>("optimize_document", {
          input: job.input,
          output: job.output,
          settings: { ...settings },
        }),
        update,
        () => stop.current,
      );
      setNotice(stop.current
        ? "停止しました。処理済みファイルは保存されています。残りは再開できます。"
        : "処理が終了しました。各ファイルの結果をご確認ください。");
    } finally {
      active.current = false;
      setBusy(false);
      setStopping(false);
    }
  }

  const pending = jobs.filter(job => job.status === "pending").length;
  const finished = jobs.filter(job => ["done", "unchanged", "error", "cancelled"].includes(job.status)).length;
  const saved = jobs.reduce((sum, job) => sum + (job.report?.saved ?? 0), 0);
  const pdfPending = jobs.some(job => job.status === "pending" && job.input.toLowerCase().endsWith(".pdf"));
  const pdfBlocked = pdfPending && (!engine?.available || !settings.pdfAcknowledged);

  return (
    <main className="app">
      <header className="app-header">
        <div>
          <h1>ファイルを、軽く。</h1>
          <p>元の文書を残して、別名で保存します。</p>
        </div>
        <label className="theme-picker">
          表示
          <select aria-label="表示テーマ" value={theme} onChange={event => setTheme(event.target.value)}>
            <option value="light">ライト</option>
            <option value="dark">ダーク</option>
          </select>
        </label>
      </header>

      <div className="workspace">
        <section className="queue" aria-labelledby="queue-title">
          <div className="section-heading">
            <h2 id="queue-title">ファイル <span className="count">{jobs.length}</span></h2>
            <button disabled={busy} onClick={addFiles}>ファイルを追加</button>
          </div>

          {jobs.length === 0 ? (
            <div className="empty">
              <h3>最適化する文書を選択</h3>
              <p>複数のファイルをまとめて処理できます。</p>
              <p className="formats">XLSX · XLSM · DOCX · PPTX · PDF</p>
              <p className="hint">Office文書は追加ソフト不要。PDFはGhostscriptが必要です。</p>
            </div>
          ) : (
            <ul className="job-list">
              {jobs.map(job => (
                <JobRow
                  key={job.id}
                  job={job}
                  busy={busy}
                  onOutput={() => chooseOutput(job)}
                  onRemove={() => removeJob(job)}
                  onRetry={() => update(job.id, { status: "pending", error: undefined, report: undefined })}
                />
              ))}
            </ul>
          )}

          <div className="queue-footer">
            <span>{finished} / {jobs.length} 件終了</span>
            <span>合計削減 <strong>{formatBytes(saved)}</strong></span>
          </div>
        </section>

        <SettingsPanel
          settings={settings}
          change={setSettings}
          busy={busy}
          engine={engine}
          directory={directory}
          chooseDirectory={chooseDirectory}
          resetDirectory={() => applyDirectory("")}
        />
      </div>

      <footer className="run-bar">
        <div className="run-message">
          <p role="status" aria-live="polite">{notice}</p>
          {pdfBlocked && <p className="warning">PDFの処理にはGhostscriptと注意事項の確認が必要です。</p>}
          {busy && <p className="hint">停止は現在のファイルの完了後に適用されます。</p>}
        </div>
        {busy ? (
          <button
            disabled={stopping}
            onClick={() => {
              stop.current = true;
              setStopping(true);
              setNotice("現在のファイルの終了を待って停止します。");
            }}
          >
            {stopping ? "停止を予約済み" : "このファイルの後で停止"}
          </button>
        ) : (
          <button className="primary" disabled={pending === 0 || pdfBlocked} onClick={start}>
            最適化を開始
          </button>
        )}
      </footer>

      <p className="footnote">処理はこのPC内で実行します。削減できない場合は元と同じ内容を別名保存します。</p>

      {dropActive && (
        <div className="drop-overlay" role="status" aria-live="polite">
          <strong>ここにファイルをドロップ</strong>
        </div>
      )}
    </main>
  );
}
