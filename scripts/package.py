#!/usr/bin/env python3
"""Package the native release binary. Run on the OS being packaged."""
import argparse
import os
from pathlib import Path
import plistlib
import re
import shutil
import subprocess
import tarfile
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parents[1]
VERSION = re.search(r'^version = "([^"]+)"', (ROOT/'Cargo.toml').read_text(), re.M)[1]
DIST = ROOT/'dist'

def run(*args):
    subprocess.run([str(a) for a in args], check=True)

def copy(src, dst):
    dst = Path(dst)
    dst.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(src, dst)

def notices(dest):
    for name in ['LICENSE', 'NOTICE', 'README.md']:
        copy(ROOT/name, dest/name)
    copy(ROOT/'assets/FONT-LICENSE', dest/'licenses/FONT-LICENSE')
    copy(ROOT/'third-party/libratbag-LICENSE', dest/'licenses/libratbag-LICENSE')

def linux(platform, scratch, binary):
    arch = 'amd64' if platform.endswith('x86_64') else 'arm64'
    app = scratch/'g502x-control'
    copy(binary, app/'g502x-control')
    (app/'g502x-control').chmod(0o755)
    notices(app)
    copy(ROOT/'packaging/install-linux.sh', app/'install.sh')
    for name in ['g502x-control.svg', '70-g502x-control.rules']:
        copy(ROOT/'packaging'/name, app/'packaging'/name)
    with tarfile.open(DIST/f'g502x-control-{platform}.tar.gz', 'w:gz') as archive:
        archive.add(app, arcname='g502x-control')
    deb = scratch/'deb'
    copy(binary, deb/'usr/bin/g502x-control')
    (deb/'usr/bin/g502x-control').chmod(0o755)
    copy(ROOT/'packaging/g502x-control.desktop', deb/'usr/share/applications/g502x-control.desktop')
    copy(ROOT/'packaging/g502x-control.svg', deb/'usr/share/icons/hicolor/scalable/apps/g502x-control.svg')
    copy(ROOT/'packaging/70-g502x-control.rules', deb/'usr/lib/udev/rules.d/70-g502x-control.rules')
    notices(deb/'usr/share/doc/g502x-control')
    control = deb/'DEBIAN'
    control.mkdir()
    (control/'control').write_text(f'''Package: g502x-control
Version: {VERSION}
Section: utils
Priority: optional
Architecture: {arch}
Maintainer: G502 X Control contributors <69633045+efaraz27@users.noreply.github.com>
Homepage: https://github.com/efaraz27/g502x-control
Depends: libc6 (>= 2.35), libgcc-s1, libstdc++6, libxkbcommon0, libxkbcommon-x11-0, libwayland-client0, libegl1, libgl1, libx11-6, libxi6, libxcursor1, libxrandr2
Recommends: zenity
Description: Native control panel for Logitech G502 X Plus
 Configure DPI, onboard profiles, buttons and lighting over USB or LIGHTSPEED.
''')
    for name in ['postinst', 'postrm']:
        script = control/name
        script.write_text('#!/bin/sh\nset -e\nif command -v udevadm >/dev/null 2>&1; then udevadm control --reload-rules || true; fi\nexit 0\n')
        script.chmod(0o755)
    run('dpkg-deb', '--build', '--root-owner-group', deb, DIST/f'g502x-control-{platform}.deb')

def macos(platform, scratch, binary):
    volume = scratch/'volume'
    app = volume/'G502 X Control.app'
    contents = app/'Contents'
    copy(binary, contents/'MacOS/g502x-control')
    (contents/'MacOS/g502x-control').chmod(0o755)
    copy(ROOT/'packaging/g502x-control.icns', contents/'Resources/g502x-control.icns')
    notices(contents/'Resources')
    with (contents/'Info.plist').open('wb') as f:
        plistlib.dump({
            'CFBundleName': 'G502 X Control',
            'CFBundleDisplayName': 'G502 X Control',
            'CFBundleExecutable': 'g502x-control',
            'CFBundleIdentifier': 'io.github.efaraz27.g502x-control',
            'CFBundleVersion': VERSION,
            'CFBundleShortVersionString': VERSION,
            'CFBundlePackageType': 'APPL',
            'CFBundleIconFile': 'g502x-control.icns',
            'LSMinimumSystemVersion': '13.0',
            'NSHighResolutionCapable': True,
            'NSHumanReadableCopyright': 'GPL-2.0-or-later. Unofficial; not affiliated with Logitech.',
        }, f)
    run('codesign', '--force', '--deep', '--sign', '-', app)
    run('codesign', '--verify', '--deep', '--strict', app)
    (volume/'Applications').symlink_to('/Applications')
    run('hdiutil', 'create', '-volname', 'G502 X Control', '-srcfolder', volume,
        '-ov', '-format', 'UDZO', DIST/f'g502x-control-{platform}.dmg')

def windows(platform, scratch, binary):
    app = scratch/'g502x-control'
    copy(binary, app/'g502x-control.exe')
    notices(app)
    with zipfile.ZipFile(DIST/f'g502x-control-{platform}.zip', 'w', zipfile.ZIP_DEFLATED) as archive:
        for path in app.rglob('*'):
            if path.is_file():
                archive.write(path, path.relative_to(scratch))
    nsis = shutil.which('makensis') or r'C:\Program Files (x86)\NSIS\makensis.exe'
    run(nsis, f'/DVERSION={VERSION}', f'/DSOURCE={ROOT}', f'/DPAYLOAD={app}',
        f'/DOUTPUT={DIST / ("g502x-control-"+platform+"-setup.exe")}', ROOT/'packaging/windows.nsi')

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--platform', required=True, choices=[
        'linux-x86_64', 'linux-aarch64', 'windows-x86_64', 'macos-x86_64', 'macos-aarch64'])
    args = parser.parse_args()
    os_name = args.platform.split('-')[0]
    filename = 'g502x-control.exe' if os_name == 'windows' else 'g502x-control'
    binary = Path(os.environ.get('CARGO_TARGET_DIR', ROOT/'target'))/'release'/filename
    if not binary.is_file():
        parser.error(f'Build first: missing {binary}')
    DIST.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='g502x-package-') as temp:
        globals()[os_name](args.platform, Path(temp), binary)
