import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { readFileSync } from "node:fs";
import App from "../src/App";

type DropEvent = { payload: { type: string; paths?: string[] } };
type CloseEvent = { preventDefault: () => void; isPreventDefault: () => boolean };
const native = vi.hoisted(() => ({
  invoke: vi.fn(),
  open: vi.fn(),
  save: vi.fn(),
  destroy: vi.fn().mockResolvedValue(undefined),
  dropHandler: null as ((event: DropEvent) => void) | null,
  closeHandler: null as ((event: CloseEvent) => Promise<void>) | null,
}));

vi.mock("@tauri-apps/api/core", () => ({ invoke: native.invoke }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: native.open, save: native.save }));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    onCloseRequested: async (handler: (event: CloseEvent) => void | Promise<void>) => {
      native.closeHandler = async event => {
        await handler(event);
        if (!event.isPreventDefault()) await native.destroy();
      };
      return () => {};
    },
    destroy: native.destroy,
  }),
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
  native.closeHandler = null;
  native.destroy.mockResolvedValue(undefined);
});

test("Tauri's default capability permits the close-request handler to destroy the window", () => {
  const capability = JSON.parse(readFileSync("src-tauri/capabilities/default.json", "utf8"));
  expect(capability.permissions).toContain("core:window:allow-destroy");
});

test("close while idle is not blocked", async () => {
  native.invoke.mockResolvedValue({ available: false, path: null, version: null });
  render(<App />);
  await waitFor(() => expect(native.closeHandler).not.toBeNull());

  let prevented = false;
  await native.closeHandler?.({
    preventDefault: () => { prevented = true; },
    isPreventDefault: () => prevented,
  });

  expect(prevented).toBe(false);
  expect(native.destroy).toHaveBeenCalledOnce();
});

test("close during processing stops after the current file and closes automatically", async () => {
  native.open.mockResolvedValue(["C:/a.xlsx", "C:/b.xlsx"]);
  let finishCurrent: ((report: Record<string, unknown>) => void) | undefined;
  native.invoke.mockImplementation(async (command: string) => {
    if (command === "pdf_engine") return { available: false, path: null, version: null };
    return new Promise(resolve => { finishCurrent = resolve; });
  });
  render(<App />);
  await waitFor(() => expect(native.closeHandler).not.toBeNull());

  fireEvent.click(screen.getByRole("button", { name: "ファイルを追加" }));
  await screen.findByText("a.xlsx");
  await screen.findByText("b.xlsx");
  fireEvent.click(screen.getByRole("button", { name: "最適化を開始" }));
  await waitFor(() => expect(native.invoke.mock.calls.some(([command]) => command === "optimize_document")).toBe(true));

  let prevented = false;
  await native.closeHandler?.({
    preventDefault: () => { prevented = true; },
    isPreventDefault: () => prevented,
  });
  expect(prevented).toBe(true);
  expect(await screen.findByText("現在のファイルが終わった後、ウィンドウを閉じます。")).toBeTruthy();

  finishCurrent?.({
    output: "C:/a_optimized.xlsx", before: 100, after: 80, saved: 20,
    optimized: 1, skipped: 0, unchanged: false, messages: [], omittedMessages: 0,
  });
  await waitFor(() => expect(native.destroy).toHaveBeenCalledOnce());
  expect(native.invoke.mock.calls.filter(([command]) => command === "optimize_document")).toHaveLength(1);
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
