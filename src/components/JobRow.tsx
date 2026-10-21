import type { Job, Status } from "../types";
const statusText: Record<Status, string> = { pending: "待機", running: "処理中", done: "完了", unchanged: "削減なし", error: "エラー", cancelled: "中止" };
export const formatBytes = (n: number) => n < 1024 ? `${n} B` : n < 1048576 ? `${(n / 1024).toFixed(1)} KB` : `${(n / 1048576).toFixed(2)} MB`;
export const filename = (path: string) => path.split(/[\\/]/).pop() ?? path;

type Props = { job: Job; busy: boolean; onOutput: () => void; onRemove: () => void; onRetry: () => void };
export function JobRow({ job, busy, onOutput, onRemove, onRetry }: Props) {
  const result = job.report;
  const canEdit = ["pending", "error", "cancelled"].includes(job.status);
  return <li className="job">
    <div className="job-heading"><span className="file-kind">{filename(job.input).split(".").pop()?.toUpperCase()}</span>
      <h3 title={job.input}>{filename(job.input)}</h3><span className={`badge ${job.status}`}>{statusText[job.status]}</span></div>
    <p className="path" title={job.input}>{job.input}</p>
    <div className="output-row"><span className="path" title={job.output}>保存先: {job.output}</span>
      {canEdit && <button className="text-button" disabled={busy} onClick={onOutput} aria-label={`${filename(job.input)} の保存先を変更`}>変更</button>}</div>
    {result && <div className="job-result"><strong>{formatBytes(result.before)} <span aria-label="から">→</span> {formatBytes(result.after)}</strong>
      <span>{result.saved > 0 ? `${formatBytes(result.saved)} 削減 (${(100 * result.saved / result.before).toFixed(1)}%)` : "元と同じ内容を別名保存"}</span>
      <small>画像 {result.optimized} 件を最適化 / {result.skipped} 件を保持</small></div>}
    {job.error && <p className="error-text" role="alert">{job.error}</p>}
    {result && result.messages.length > 0 && <details><summary>処理の詳細・保持した画像</summary><ul className="messages">{result.messages.map((message, index) => <li key={index}>{message}</li>)}{result.omittedMessages > 0 && <li>ほか {result.omittedMessages} 件（表示上限100件）</li>}</ul></details>}
    <div className="job-actions">{["error", "cancelled"].includes(job.status) && <button className="text-button" disabled={busy} onClick={onRetry}>待機に戻す</button>}
      <button className="text-button" disabled={busy} onClick={onRemove} aria-label={`${filename(job.input)} を一覧から削除`}>一覧から削除</button></div>
  </li>;
}
