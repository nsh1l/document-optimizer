# Document Optimizer 1.0.0

Windows x64向けのローカル文書軽量化ツール。React / TypeScriptの日本語GUIとTauri / Rustの処理エンジンを使用します。文書をサーバーへ送信しません。

## 使い方

1. EXEを起動するか、NSIS / MSIインストーラーでインストールします。Microsoft Edge WebView2 Runtimeが必要です（ランタイム導入にはネット接続が必要になる場合があります）。
2. 「ファイルを追加」またはドラッグ＆ドロップで文書を追加します。複数ファイルもまとめて登録できます。
3. 保存先を確認します。既定は元と同じフォルダーの `元の名前_optimized.拡張子`。各項目の保存先も変更できます。
4. 必要に応じてJPEG再圧縮やPDF品質を選び、「最適化を開始」を押します。
5. 削減量・保持理由・エラーを確認し、出力を元のアプリで開いて確認してください。

元ファイルも既存の出力ファイルも上書きしません。削減できない場合は元と同一内容を別名保存します。

## 対応する処理

- **XLSX / XLSM / DOCX / PPTX**: `xl/media/`、`word/media/`、`ppt/media/`内のPNGをOxiPNGで可逆最適化。XML・リレーションシップ・画像名・画像形式は書き換えません。変更対象外のZIP項目は圧縮済みデータをコピーし、マクロのバイト列も保持します。
- **JPEG**: 既定は無効。明示的に有効化した場合のみ品質50〜95（既定85）で非可逆再圧縮します。JPEGのまま保持し、PNGには変換しません。追加メタデータや複数画像を含むJPEGは保守的にスキップします。
- **その他の画像**: GIF、APNG、BMP、TIFF、SVG、EMF、WMF、WebPなどは変換せず保持します。壊れた画像や制限超過も保持理由を表示します。
- **PDF**: 別途導入したGhostscriptのpdfwriteで再生成し、生成結果をGhostscriptで読み込み検証。screen / ebook / prepressを選べます。PDFヘッダーのない入力は外部エンジン起動前に拒否します。
- 逐次キュー、個別エラー・再試行・削除、保存先選択、ライト / ダーク表示。テーマのみ保存し、キューと処理設定は再起動でリセットします。

## PDFの依存関係・注意

Ghostscriptはアプリにもインストーラーにも同梱しません。64-bit版の `gswin64c.exe` をPATH、または次の場所に導入してアプリを再起動してください。

```text
C:\Program Files\gs\<version>\bin\gswin64c.exe
```

画面上の注意事項への同意が必要です。PDF再生成は可逆最適化ではありません。**電子署名・フォーム・注釈・色・アクセシビリティなどの保持は保証しません。** 読込検証は見た目や機能の同一性を証明しません。原本と比較してください。署名付きPDFを自動判別して保持する機能はありません。Ghostscriptの利用・再配布条件は別途確認してください。

## 安全策と制限

- 隣接一時ファイルで処理し、完了後に既存出力を上書きせず公開。通常のエラーでは片付けますが、強制終了・電源断では一時ファイルが残る場合があります。
- ZIPを処理前後に全項目読込検証し、CRCエラー・重複項目・項目一覧不一致を拒否。`_xmlsignatures/`を持つ署名付きOfficeパッケージは拒否します。
- 入力は最大1 GiB。ZIPは最大50,000項目、各展開256 MiB、合計展開2 GiB。最適化対象画像は圧縮状態32 MiB以下で、画像展開には128 MiBの制限を設定しています。
- PNGに20秒の時間予算、Ghostscript再生成・検証にそれぞれ300秒、診断ログに4 MiBの上限を設定しています。
- 「このファイルの後で停止」は実行中のファイル完了後に残りを中止します。即時キャンセルではありません。処理中の通常のウィンドウ終了は抑止します。
- ファイル内部の進捗率、履歴保存、自動更新、クラウド連携、画像リサイズはありません。ドラッグ＆ドロップは対応拡張子のファイルのみ受け付けます。
- OOXMLスキーマの完全検証やMicrosoft Officeによる互換性検証は行いません。拡張子とZIP構造を受け付けるため、真正なOffice文書であることは保証しません。旧形式XLS / DOC / PPT、暗号化Office、DOCM / PPTMは対象外です。
- 削減率は文書によります。PNGの少ない文書では削減されないことがあります。
- コード署名していません。Windowsが発行元未確認の警告を表示する場合があります。クリーン環境でのインストール・アンインストールは未検証です。

## ソース構成

- `src/App.tsx` / `src/components/`: キュー画面、設定、ファイル行
- `src/queue.ts`: テスト可能なキュー制御
- `src-tauri/src/lib.rs`: Tauriコマンド・単一ワーカー制御
- `src-tauri/src/engine/mod.rs`: 入出力検証、安全な公開、サイズ比較
- `src-tauri/src/engine/{office,images,pdf}.rs`: ZIP検証、画像処理、Ghostscript制御
- `src-tauri/src/engine/tests.rs`: 合成フィクスチャの回帰テスト
- `scripts/build_windows.py`: この開発機用MSVC / SDK環境ラッパー

## ビルド・テスト

Node.js / npm、Rust MSVCターゲット、Visual Studio C++ Build Tools、Windows SDK、WebView2が必要です。通常のDeveloper Command Prompt:

```bash
npm ci
npm test
npm run build
cargo test --manifest-path src-tauri/Cargo.toml --lib
# Ghostscript導入済み環境のみ（外部エンジンの実テストを含む）
cargo test --manifest-path src-tauri/Cargo.toml --lib -- --include-ignored --nocapture
npm run tauri build
```

この開発機では `python scripts/build_windows.py npm test`、`python scripts/build_windows.py cargo test --lib -- --include-ignored --nocapture`、`python scripts/build_windows.py` を利用できます。ラッパーはこの機械のMSVC / SDK / Node配置に依存するため、他の機械では内容を調整するか、標準のDeveloper Command Promptを利用してください。MSVCの `link.exe` がGit Bashの同名コマンドより先に解決される必要があります。

成果物は `src-tauri/target/release/document_optimizer.exe` と `src-tauri/target/release/bundle/` 以下。ソースZIPには `node_modules`、`dist`、`target`、過去の配布物を含めません。

テストは合成フィクスチャを使用します。実業務文書の全機能・見た目を網羅した保証ではありません。
