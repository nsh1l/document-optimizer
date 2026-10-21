export type Settings = {
  jpegLossy: boolean;
  jpegQuality: number;
  pdfQuality: "screen" | "ebook" | "prepress";
  pdfAcknowledged: boolean;
};
export type Report = {
  output: string;
  before: number;
  after: number;
  saved: number;
  optimized: number;
  skipped: number;
  unchanged: boolean;
  messages: string[];
  omittedMessages: number;
};
export type Status = "pending" | "running" | "done" | "unchanged" | "error" | "cancelled";
export type Job = { id: string; input: string; output: string; status: Status; report?: Report; error?: string };
export type PdfEngine = { available: boolean; path: string | null; version: string | null };
export const defaultSettings: Settings = { jpegLossy: false, jpegQuality: 85, pdfQuality: "ebook", pdfAcknowledged: false };
