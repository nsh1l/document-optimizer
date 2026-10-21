"""Reproducible local Windows toolchain entry point; no shell expansion."""
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
env = os.environ.copy()
home = Path.home()
msvc = Path('C:/BuildTools/VC/Tools/MSVC/14.44.35207')
sdk = Path('C:/Program Files (x86)/Windows Kits/10')
version = '10.0.26100.0'
node = home / 'AppData/Local/hermes/node'
env['PATH'] = os.pathsep.join(map(str, [msvc / 'bin/Hostx64/x64', home / '.cargo/bin', node])) + os.pathsep + env['PATH']
env['LIB'] = ';'.join(map(str, [home / 'msvc_libs/x64', sdk / f'Lib/{version}/um/x64', sdk / f'Lib/{version}/ucrt/x64']))
env['INCLUDE'] = ';'.join(map(str, [msvc / 'include', sdk / f'Include/{version}/ucrt', sdk / f'Include/{version}/shared', sdk / f'Include/{version}/um', sdk / f'Include/{version}/winrt']))
args = sys.argv[1:]
if not args:
    args = ['npm', 'run', 'tauri', 'build']
if args[0] == 'npm':
    args = [str(node / 'node.exe'), str(node / 'node_modules/npm/bin/npm-cli.js'), *args[1:]]
if args[0] == 'cargo':
    args[0] = str(home / '.cargo/bin/cargo.exe')
    cwd = ROOT / 'src-tauri'
else:
    cwd = ROOT
raise SystemExit(subprocess.call(args, cwd=cwd, env=env))
