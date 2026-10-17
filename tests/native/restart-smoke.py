"""Verify authenticated restart relaunches the portable EXE with saved state."""
import ctypes
from ctypes import wintypes
import json
from pathlib import Path
import shutil
import sys
import urllib.error
import uuid

from smoke import ROOT, Running, USER32, wait_for, windows_for

USER32.FindWindowW.argtypes = [wintypes.LPCWSTR, wintypes.LPCWSTR]
USER32.FindWindowW.restype = wintypes.HWND
assert not USER32.FindWindowW('ScheduledScreenshot.Instance', None), 'Exit the running app before testing'
folder = ROOT / 'artifacts' / ('restart smoke 截圖 ' + uuid.uuid4().hex[:8])
folder.mkdir(parents=True)
source = Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / 'target/x86_64-pc-windows-msvc/debug/ScheduledScreenshot.exe'
shutil.copy2(source, folder / 'ScheduledScreenshot.exe')
app = Running(folder)
replacement = None
try:
    initial = app.status()
    settings = initial['settings']
    settings['tasks'] = [initial['defaultTask']]
    settings['paused'] = True
    settings['manualOutputFolder'] = 'restart captures'
    app.save(settings)
    progress = app.status()['tasks'][0]['progress']
    saved = (folder / 'data/settings.json').read_bytes()
    for body, token, expected in [({}, 'invalid-token', 403), ({'unexpected': True}, app.token, 400)]:
        try:
            app.request('restart', body, token=token)
            raise AssertionError('Invalid restart request was accepted')
        except urllib.error.HTTPError as error:
            assert error.code == expected
        assert app.process.poll() is None
    assert app.request('restart') == {'accepted': True}
    assert app.process.wait(timeout=40) == 0
    def new_window():
        hwnd = USER32.FindWindowW('ScheduledScreenshot.Instance', None)
        if not hwnd:
            return None
        pid = wintypes.DWORD()
        USER32.GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
        return (hwnd, pid.value) if pid.value != app.process.pid else None
    replacement = wait_for(new_window)
    lines = wait_for(lambda: (lines if len(lines) >= 2 else None)
                     if (lines := (folder / 'stdout.txt').read_text(encoding='utf-8').splitlines()) else None)
    app.origin, app.token = lines[-1].split('/#token=')
    status = app.status()
    assert status['settings']['paused'] is True
    assert status['settings']['manualOutputFolder'] == 'restart captures'
    assert status['tasks'][0]['progress'] == progress
    assert (folder / 'data/settings.json').read_bytes() == saved
    print(json.dumps({'restart': 'passed', 'oldPid': app.process.pid, 'newPid': replacement[1]}))
finally:
    if replacement:
        USER32.PostMessageW(replacement[0], 0x0010, 0, 0)
        wait_for(lambda: not windows_for(replacement[1], 'Scheduled Screenshot'))
    app.close()
