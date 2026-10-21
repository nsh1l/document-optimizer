"""Package built artifacts and source; verify archive exclusions and hash copies."""
from pathlib import Path
import hashlib
import json
import shutil
import zipfile

ROOT = Path(__file__).resolve().parents[1]
DEST = ROOT.parent / 'DocumentOptimizer-1.0.0-release'
DEST.mkdir(exist_ok=True)
release = ROOT / 'src-tauri/target/release'
artifacts = [release / 'document_optimizer.exe',
             release / 'bundle/nsis/Document Optimizer_1.0.0_x64-setup.exe',
             release / 'bundle/msi/Document Optimizer_1.0.0_x64_en-US.msi']
for source in artifacts:
    assert source.is_file() and source.stat().st_size > 100_000, source
    shutil.copy2(source, DEST / source.name)
shutil.copy2(ROOT / 'README.md', DEST / 'README.md')
if (ROOT / 'VERIFICATION.md').exists():
    shutil.copy2(ROOT / 'VERIFICATION.md', DEST / 'VERIFICATION.md')
excluded = {'node_modules', 'target', 'dist', '.git', '.vs', '__pycache__', 'release', 'artifacts'}
archive_path = DEST / 'DocumentOptimizer-1.0.0-source.zip'
with zipfile.ZipFile(archive_path, 'w', zipfile.ZIP_DEFLATED) as archive:
    import os
    for directory, dirs, files in os.walk(ROOT):
        dirs[:] = sorted(d for d in dirs if d not in excluded)
        for name in sorted(files):
            path = Path(directory) / name
            rel = path.relative_to(ROOT)
            if path.suffix.lower() in {'.zip', '.exe', '.msi', '.pdb', '.log', '.pyc'}:
                continue
            archive.write(path, Path('DocumentOptimizer') / rel)
with zipfile.ZipFile(archive_path) as archive:
    assert archive.testzip() is None
    names = archive.namelist()
    assert not any(set(Path(name).parts) & excluded for name in names)
    for required in ['package-lock.json', 'src-tauri/Cargo.lock', 'README.md', 'src-tauri/src/engine/pdf.rs']:
        assert 'DocumentOptimizer/' + required in names

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()
for source in artifacts:
    assert digest(source) == digest(DEST / source.name)
manifest = {path.name: {'bytes': path.stat().st_size, 'sha256': digest(path)}
            for path in sorted(DEST.iterdir()) if path.is_file() and path.name != 'SHA256.json'}
(DEST / 'SHA256.json').write_text(json.dumps(manifest, indent=2), encoding='utf-8')
print(json.dumps({'directory': str(DEST), 'source_entries': len(names), 'files': manifest}, indent=2))
