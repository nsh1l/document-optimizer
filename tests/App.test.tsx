import { afterEach, expect, test, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import App from '../src/App';
const native = vi.hoisted(() => ({ invoke: vi.fn(), open: vi.fn(), save: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: native.invoke }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: native.open, save: native.save }));
vi.mock('@tauri-apps/api/window', () => ({ getCurrentWindow: () => ({ onCloseRequested: async () => () => {} }) }));
afterEach(() => { cleanup(); vi.resetAllMocks(); });
test('safe defaults are visible, start is disabled and PDF dependency is honest', async () => {
  native.invoke.mockResolvedValue({ available: false, path: null, version: null });
  render(<App />);
  expect((screen.getByRole('button', {name: '最適化を開始'}) as HTMLButtonElement).disabled).toBe(true);
  expect((screen.getByLabelText('JPEGを再圧縮する（非可逆）') as HTMLInputElement).checked).toBe(false);
  await waitFor(() => expect(screen.getByText(/Ghostscript 未検出/)).toBeTruthy());
});
test('multiple files queue, genuine native error, and continued processing', async () => {
  native.open.mockResolvedValue(['C:/a.xlsx','C:/b.docx']);
  native.invoke.mockImplementation(async (command: string, args?: {input: string}) => {
    if(command==='pdf_engine') return {available:false,path:null,version:null};
    if(args?.input==='C:/a.xlsx') throw new Error('existing output');
    return {output:'C:/b_optimized.docx',before:100,after:80,saved:20,optimized:1,skipped:0,unchanged:false,messages:[],omittedMessages:0};
  });
  render(<App />);
  fireEvent.click(screen.getByRole('button',{name:'ファイルを追加'}));
  await screen.findByText('a.xlsx'); await screen.findByText('b.docx');
  fireEvent.click(screen.getByRole('button',{name:'最適化を開始'}));
  await screen.findByText(/existing output/); await screen.findByText('完了');
  const calls=native.invoke.mock.calls.filter(([cmd])=>cmd==='optimize_document');
  expect(calls).toHaveLength(2);
  expect(calls[0][1].settings.jpegLossy).toBe(false);
  expect(calls[0][1].output).toBe('C:/a_optimized.xlsx');
});
