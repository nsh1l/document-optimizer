import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import App from "../src/App";

type DropEvent = { payload: { type: string; paths?: string[] } };
const native = vi.hoisted(() => ({
  invoke: vi.fn(),
  open: vi.fn(),
  save: vi.fn(),
  dropHandler: null as ((event: DropEvent) => void) | null,
}));

vi.mock("@tauri-apps/api/core", () => ({ invoke: native.invoke }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: native.open, save: native.save }));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ onCloseRequested: async () => () => {} }),
}));
vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({
    onDragDropEvent: async (handler: (event: DropEvent) => void) => {
      native.dropHandler = handler;
      return () => {};
    },
  }),
}));

afterEach(() => {
  cleanup();
  vi.resetAllMocks();
  native.dropHandler = null;
});

test("safe defaults are visible, start is disabled and PDF dependency is honest", async () => {
  native.invoke.mockResolvedValue({ available: false, path: null, version: null });
  render(<App />);

  expect((screen.getByRole("button", { name: "最適化を開始" }) as HTMLButtonElement).disabled).toBe(true);
  expect((screen.getByLabelText("JPEGを再圧縮する（非可逆）") as HTMLInputElement).checked).toBe(false);
  expect(screen.queryByText("Document Optimizer")).toBeNull();
  await waitFor(() => expect(screen.getByText(/Ghostscript 未検出/)).toBeTruthy());
});

test("drag and drop queues supported files and reports ignored extensions", async () => {
  native.invoke.mockResolvedValue({ available: false, path: null, version: null });
  render(<App />);
  await waitFor(() => expect(native.dropHandler).not.toBeNull());

  native.dropHandler?.({
    payload: { type: "drop", paths: ["C:/資料/企画.PPTX", "C:/資料/メモ.txt"] },
  });

  await screen.findByText("企画.PPTX");
  expect(screen.getByText(/1件を追加・対応外 1件/)).toBeTruthy();
  expect(screen.getByRole("button", { name: "最適化を開始" })).toBeTruthy();
});

test("multiple files queue, native errors are reported, and later jobs continue", async () => {
  native.open.mockResolvedValue(["C:/a.xlsx", "C:/b.docx"]);
  native.invoke.mockImplementation(async (command: string, args?: { input: string }) => {
    if (command === "pdf_engine") return { available: false, path: null, version: null };
    if (args?.input === "C:/a.xlsx") throw new Error("existing output");
    return {
      output: "C:/b_optimized.docx",
      before: 100,
      after: 80,
      saved: 20,
      optimized: 1,
      skipped: 0,
      unchanged: false,
      messages: [],
      omittedMessages: 0,
    };
  });
  render(<App />);

  fireEvent.click(screen.getByRole("button", { name: "ファイルを追加" }));
  await screen.findByText("a.xlsx");
  await screen.findByText("b.docx");
  fireEvent.click(screen.getByRole("button", { name: "最適化を開始" }));
  await screen.findByText(/existing output/);
  await screen.findByText("完了");

  const calls = native.invoke.mock.calls.filter(([command]) => command === "optimize_document");
  expect(calls).toHaveLength(2);
  expect(calls[0][1].settings.jpegLossy).toBe(false);
  expect(calls[0][1].output).toBe("C:/a_optimized.xlsx");
});
