# Release verification — Document Optimizer 1.0.0

## Executed successfully

- Reproduced the existing `non_pdf_payload_is_rejected_before_invoking_external_engine` failure: Ghostscript accepted a 13-byte PostScript payload, after which the no-growth fallback saved it as a PDF.
- Added input `%PDF-` header validation before Ghostscript detection/invocation. The regression now passes.
- `python scripts/build_windows.py cargo test --lib -- --include-ignored --nocapture`: **22 passed, 0 failed, 0 ignored**. Repeated after `cargo fmt`: same result.
- Real Ghostscript **10.07.1** integration: synthetic PDF **30,451 → 2,554 bytes**; source unchanged; temporary files cleaned. Invalid synthetic PDF returned Ghostscript diagnostics and left no output.
- `python scripts/build_windows.py npm test`: **3 queue tests + 2 UI tests passed**.
- `python scripts/build_windows.py npm run build`: TypeScript and Vite production build passed.
- `python scripts/build_windows.py cargo clippy --lib -- -D warnings`: passed.
- `python scripts/build_windows.py cargo fmt`: completed.
- `python scripts/build_windows.py`: release build and **NSIS + MSI bundles completed with exit code 0**.
- Launched the actual `src-tauri/target/release/document_optimizer.exe`. Native window capture showed the Japanese queue/settings GUI rendered, light theme, and detected Ghostscript 10.07.1. No development server was used.

## Verification boundaries

- The attempted native Add Files click required interactive tool approval, so a complete end-to-end native dialog/optimization run was **not performed**. Automated UI tests and engine tests passed independently; they are not a substitute for that unperformed check.
- Installers were built, not installed/uninstalled in a clean Windows environment. They are unsigned.
- Office tests use synthetic ZIP/image fixtures. No Microsoft Office visual comparison or broad real-world document corpus validation was performed.
- PDF integration validates processing, parsing, cleanup and source preservation, not visual/form/signature/accessibility equivalence.
- The working directory is not a Git repository, so there is no Git diff or commit history to report.

## Deliverables

`C:/Users/NT22B083U/DocumentOptimizer-1.0.0-release/` contains the release EXE, NSIS installer, MSI installer, source ZIP, README, this report and SHA256.json. `scripts/package_release.py` checks copies against build artifacts, validates ZIP CRCs and required source entries, and asserts exclusions for build/dependency trees. It excludes prior ZIP/EXE/MSI archives.

`scripts/make_smoke_fixture.py` optionally creates a synthetic DOCX in the Hermes scratch directory for manual dialog testing; it requires Python and Pillow. It does not read user documents.
