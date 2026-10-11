#!/usr/bin/env python3
"""Smoke-test the Windows installer and portable release on a Windows runner."""
import os
import json
import time
from pathlib import Path
import struct
import subprocess
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parents[1]

def imports(path):
    data = path.read_bytes()
    pe = struct.unpack_from('<I', data, 0x3c)[0]
    assert data[pe:pe+4] == b'PE\0\0'
    count = struct.unpack_from('<H', data, pe+6)[0]
    size = struct.unpack_from('<H', data, pe+20)[0]
    optional = pe+24
    assert struct.unpack_from('<H',data,optional)[0] == 0x20b, 'Expected PE32+'
    sections = optional+size
    def offset(rva):
        for i in range(count):
            virtual_size, start, raw_size, raw = struct.unpack_from('<IIII',data,sections+i*40+8)
            if start <= rva < start+max(virtual_size,raw_size):
                return raw+rva-start
        raise AssertionError(f'Unmapped RVA {rva:x}')
    import_rva = struct.unpack_from('<I',data,optional+112+8)[0]
    cursor = offset(import_rva)
    names = []
    while any(data[cursor:cursor+20]):
        name_rva = struct.unpack_from('<I',data,cursor+12)[0]
        start = offset(name_rva)
        names.append(data[start:data.index(b'\0',start)].decode('ascii'))
        cursor += 20
    return names

with tempfile.TemporaryDirectory(prefix='g502x-install-') as temp:
    temp = Path(temp)
    with zipfile.ZipFile(ROOT/'dist/g502x-control-windows-x86_64.zip') as archive:
        archive.extractall(temp/'portable')
    portable = temp/'portable/g502x-control/g502x-control.exe'
    deps = imports(portable)
    assert not any(name.lower().startswith(('vcruntime','msvcp')) for name in deps), deps
    assert 'g502x-control ' in subprocess.check_output([str(portable),'--version'],text=True)
    installed = temp/'installed'
    setup = ROOT/'dist/g502x-control-windows-x86_64-setup.exe'
    # NSIS /D must be the last argument and its directory must be unquoted.
    command = subprocess.list2cmdline([str(setup),'/S'])+' /D='+str(installed)
    env = dict(os.environ, G502X_STATE_DIR=str(temp/'state'))
    subprocess.run(command, env=env, check=True, timeout=60)
    assert (installed/'g502x-control.exe').is_file()
    assert 'g502x-control ' in subprocess.check_output([str(installed/'g502x-control.exe'),'--version'],text=True)
    assert (installed/'Uninstall.exe').is_file()
    def cli(*args):
        return subprocess.check_output([str(portable), *args], env=env, text=True, timeout=30)
    def wait_running():
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline:
            status = json.loads(cli('monitor', 'status'))
            if status['running']:
                return status
            time.sleep(.1)
        raise AssertionError('Installed monitor did not restart')
    try:
        subprocess.run([str(installed/'g502x-control.exe'), 'monitor', 'start', '--at-login'], env=env, check=True, timeout=30)
        wait_running()
        subprocess.run(command, env=env, check=True, timeout=60)
        current = wait_running()
        assert current['preferences']['background_enabled'] and current['startup_registered']
    except BaseException:
        cli('monitor', 'stop')
        raise
    command = subprocess.list2cmdline([str(installed/'Uninstall.exe'),'/S'])+' _?='+str(installed)
    subprocess.run(command,env=env,check=True,timeout=60)
    assert not (installed/'g502x-control.exe').exists()
    current = json.loads(cli('monitor', 'status'))
    assert not current['running'] and not current['startup_registered']
    print('PASS: portable version, static CRT, silent install, running-monitor upgrade, preserved startup and uninstall cleanup')
