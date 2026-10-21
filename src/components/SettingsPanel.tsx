import type { PdfEngine, Settings } from "../types";
type Props = { settings: Settings; change: (settings: Settings) => void; busy: boolean; engine: PdfEngine | null; directory: string; chooseDirectory: () => void; resetDirectory: () => void };
export function SettingsPanel({ settings, change, busy, engine, directory, chooseDirectory, resetDirectory }: Props) {
  return <aside className="settings" aria-labelledby="settings-title"><h2 id="settings-title">処理設定</h2>
    <fieldset disabled={busy}><legend>Office文書</legend><p>PNGは画質を変えずに最適化します。XML・画像名・画像形式は変更しません。</p>
      <label className="checkbox"><input type="checkbox" checked={settings.jpegLossy} onChange={e => change({ ...settings, jpegLossy: e.target.checked })} />JPEGを再圧縮する（非可逆）</label>
      {settings.jpegLossy && <><label className="field-label" htmlFor="jpeg-quality">JPEG品質 <output>{settings.jpegQuality}</output></label><input id="jpeg-quality" type="range" min="50" max="95" value={settings.jpegQuality} onChange={e => change({ ...settings, jpegQuality: Number(e.target.value) })} /><p className="warning">画質が変わります。色・撮影情報付きのJPEGは保持します。</p></>}
      <p className="hint">GIF・APNG・SVGなど対象外の画像はそのまま保持します。</p>
    </fieldset>
    <fieldset disabled={busy}><legend>保存先</legend><p className="directory" title={directory}>{directory || "元ファイルと同じフォルダー"}</p><div className="button-row"><button onClick={chooseDirectory}>フォルダーを選択</button>{directory && <button className="text-button" onClick={resetDirectory}>元に戻す</button>}</div><p className="hint">末尾に _optimized を付けます。既存のファイルは上書きしません。</p></fieldset>
    <fieldset disabled={busy}><legend>PDF（外部エンジン）</legend>
      <p className="engine-state" title={engine?.path ?? undefined}>{engine === null ? "Ghostscript 確認中…" : engine.available ? `Ghostscript ${engine.version}` : "Ghostscript 未検出 — PDFは利用できません"}</p>
      <label className="field-label" htmlFor="pdf-quality">PDF品質</label><select id="pdf-quality" value={settings.pdfQuality} onChange={e => change({ ...settings, pdfQuality: e.target.value as Settings["pdfQuality"] })}>
        <option value="screen">低画質 / screen</option><option value="ebook">標準 / ebook</option><option value="prepress">高画質 / prepress</option></select>
      <p className="warning">PDFは再生成されます。署名・フォーム・注釈・色・アクセシビリティが変わる可能性があります。</p>
      <label className="checkbox"><input type="checkbox" checked={settings.pdfAcknowledged} onChange={e => change({ ...settings, pdfAcknowledged: e.target.checked })} />PDFの注意事項を確認しました</label>
      {!engine?.available && <p className="hint">Ghostscript 64-bit版を別途導入し、アプリを再起動してください。インストーラーには含まれません。</p>}
    </fieldset>
  </aside>;
}
