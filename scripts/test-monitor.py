#!/usr/bin/env python3
"""Isolated monitor/startup lifecycle check. Does not change mouse settings."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
name = 'g502x-control.exe' if sys.platform == 'win32' else 'g502x-control'
binary = Path(os.environ.get('CARGO_TARGET_DIR', ROOT / 'target')) / 'release' / name

def until(check, timeout=15):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if check():
            return
        time.sleep(.1)
    raise AssertionError('Monitor lifecycle transition timed out')

with tempfile.TemporaryDirectory(prefix='g502x monitor test ') as temp:
    state = Path(temp) / 'state'
    env = dict(os.environ, G502X_STATE_DIR=str(state), G502X_AUTOSTART_DIR=str(Path(temp)/'startup'))
    def run(*args):
        return subprocess.check_output([str(binary), *args], env=env, text=True, timeout=30)
    def status():
        return json.loads(run('monitor', 'status'))
    assert not status()['running']
    # A login invocation must do nothing until monitoring has been enabled.
    run('--background')
    assert not status()['running']
    try:
        run('monitor', 'start', '--at-login')
        until(lambda: status()['running'])
        current = status()
        assert current['preferences']['background_enabled']
        assert current['preferences']['launch_at_login'] and current['startup_registered']
        # A second launch exits instead of creating another resident process.
        run('--background')
        assert status()['running']
        run('monitor', 'quit')
        assert not status()['running']
        assert status()['preferences']['launch_at_login']
        child = subprocess.Popen([str(binary), '--background'], env=env)
        try:
            until(lambda: status()['running'])
            run('monitor', 'stop')
            child.wait(timeout=25)
            assert child.returncode == 0
        finally:
            if child.poll() is None:
                run('monitor', 'stop')
                child.wait(timeout=25)
        current = status()
        assert not current['running']
        assert not current['preferences']['background_enabled']
        assert not current['preferences']['launch_at_login']
        assert not current['startup_registered']
        print('PASS: disabled login, opt-in startup, singleton, session quit, relaunch and persistent stop')
    finally:
        run('monitor', 'stop')
