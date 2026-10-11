#!/usr/bin/env python3
"""Exercise Linux per-user installation without changing the user's home."""
import hashlib
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile

ROOT = Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='g502x install test ') as temp:
    scratch = Path(temp)
    archives = list((ROOT/'dist').glob('g502x-control-linux-*.tar.gz'))
    assert len(archives) == 1, 'Run packaging for one native Linux architecture first'
    with tarfile.open(archives[0]) as archive:
        archive.extractall(scratch/'extracted', filter='data')
    app = scratch/'extracted/g502x-control'
    prefix = scratch/'prefix with spaces'
    env = dict(os.environ, G502X_INSTALL_PREFIX=str(prefix))
    for _ in range(2):  # Verify first install and replacement/update.
        subprocess.run(['sh', str(app/'install.sh')], env=env, check=True)
    installed = prefix/'bin/g502x-control'
    assert hashlib.sha256(installed.read_bytes()).digest() == hashlib.sha256((app/'g502x-control').read_bytes()).digest()
    version = subprocess.check_output([str(installed), '--version'], text=True)
    assert version.startswith('g502x-control '), version
    desktop = prefix/'share/applications/g502x-control.desktop'
    assert f'Exec="{installed}"' in desktop.read_text()
    assert (prefix/'share/g502x-control/70-g502x-control.rules').is_file()
    # A corrupt downloaded archive must be rejected before invoking its installer.
    mocks = scratch/'mocks'
    mocks.mkdir()
    curl = mocks/'curl'
    curl.write_text('''#!/bin/sh
set -eu
while [ "$#" -gt 0 ]; do
 case "$1" in
  https:*) url=$1 ;;
  -o) shift; dest=$1 ;;
 esac
 shift
done
case "$url" in
 */SHA256SUMS.txt) printf '%064d  g502x-control-linux-%s.tar.gz\\n' 0 "$(uname -m | sed s/arm64/aarch64/)" > "$dest" ;;
 *) printf 'corrupt archive' > "$dest" ;;
esac
''')
    curl.chmod(0o755)
    invalid_prefix = scratch/'must not install'
    env.update(PATH=str(mocks)+os.pathsep+os.environ['PATH'], G502X_INSTALL_PREFIX=str(invalid_prefix))
    failed = subprocess.run(['sh', str(ROOT/'install.sh')], env=env, capture_output=True, text=True)
    assert failed.returncode != 0, 'Corrupt archive unexpectedly accepted'
    assert not invalid_prefix.exists(), 'Installer ran before checksum verification'
    print('PASS: first install, update, paths with spaces, version and corrupt-download rejection')
