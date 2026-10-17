"""Verify native global capture shortcuts in an isolated packaged instance."""
import argparse
import ctypes
from ctypes import wintypes
import json
from pathlib import Path
import shutil
import time
import uuid
from smoke import ROOT, Running, USER32, wait_for


class KeyboardInput(ctypes.Structure):
    _fields_ = [('vk', wintypes.WORD), ('scan', wintypes.WORD),
                ('flags', wintypes.DWORD), ('time', wintypes.DWORD),
                ('extra', ctypes.c_size_t)]


class InputData(ctypes.Union):
    _fields_ = [('keyboard', KeyboardInput), ('padding', ctypes.c_byte * 32)]


class Input(ctypes.Structure):
    _fields_ = [('type', wintypes.DWORD), ('data', InputData)]


USER32.SendInput.argtypes = [wintypes.UINT, ctypes.POINTER(Input), ctypes.c_int]
USER32.SendInput.restype = wintypes.UINT
USER32.RegisterHotKey.argtypes = [wintypes.HWND, ctypes.c_int, wintypes.UINT, wintypes.UINT]
USER32.UnregisterHotKey.argtypes = [wintypes.HWND, ctypes.c_int]
USER32.GetAsyncKeyState.argtypes = [ctypes.c_int]
USER32.GetAsyncKeyState.restype = ctypes.c_short
MODIFIERS = 0x0001 | 0x0002 | 0x0004
RESERVATION = 99


def key_event(key, released=False):
    event = Input(1, InputData(keyboard=KeyboardInput(key, 0, 2 if released else 0, 0, 0)))
    assert USER32.SendInput(1, ctypes.byref(event), ctypes.sizeof(Input)) == 1


def shortcut(app, key, repeats=0):
    # Never inject a chord unless this test instance owns the registration.
    assert app.status()['shortcutActive'] == f'Ctrl+Alt+Shift+F{key - 0x70 + 1}'
    keys = [0x11, 0x12, 0x10, key]
    assert all(not USER32.GetAsyncKeyState(k) & 0x8000 for k in keys), 'Release modifier keys before running this check.'
    try:
        for item in keys:
            key_event(item)
        for _ in range(repeats):
            time.sleep(.2)
            key_event(key)
    finally:
        for item in reversed(keys):
            key_event(item, released=True)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--exe', type=Path, required=True)
    args = parser.parse_args()
    folder = ROOT / 'artifacts' / ('hotkey-smoke-' + uuid.uuid4().hex[:8])
    (folder / 'data').mkdir(parents=True)
    shutil.copy2(args.exe, folder / 'ScheduledScreenshot.exe')
    (folder / 'data/settings.json').write_text(json.dumps({
        'schemaVersion': 2, 'paused': True, 'tasks': [],
        'manualOutputFolder': 'shortcut shots/\u622a\u5716',
        'captureShortcut': {'enabled': True, 'modifiers': 'ctrl+alt+shift', 'key': 'F9'}
    }), encoding='utf-8')
    checks = []
    app = Running(folder)
    reserved = False
    try:
        wait_for(lambda: app.status()['shortcutActive'] == 'Ctrl+Alt+Shift+F9')
        before = app.status()
        shortcut(app, 0x78, repeats=5)
        wait_for(lambda: app.status()['manualCapture'] and not app.status()['busy'])
        after = app.status()
        assert after['manualCapture']['success'], after
        assert after['settings'] == before['settings'] and after['tasks'] == []
        images = list((folder / 'shortcut shots').rglob('*.jpg'))
        assert len(images) == len(after['manualCapture']['files'])
        assert all(Path(name).is_file() for name in after['manualCapture']['files'])
        checks.append('actual global chord captures without a browser or tasks while paused; saved Unicode folder; held-key repeats suppressed')

        # Own a competing key on this test driver's thread, then request it in the app.
        reserved = bool(USER32.RegisterHotKey(None, RESERVATION, MODIFIERS, 0x79))
        assert reserved, 'Test conflict key is already in use.'
        settings = app.status()['settings']; settings['captureShortcut']['key'] = 'F10'
        app.save(settings)
        wait_for(lambda: app.status()['shortcutError'])
        assert app.status()['shortcutActive'] is None
        assert USER32.RegisterHotKey(None, 100, MODIFIERS, 0x78), 'Previous key was not released.'
        assert USER32.UnregisterHotKey(None, 100)
        checks.append('conflicting shortcut reports unavailable without taking over another registration; old shortcut released')

        settings = app.status()['settings']; settings['captureShortcut']['key'] = 'F8'
        app.save(settings)
        wait_for(lambda: app.status()['shortcutActive'] == 'Ctrl+Alt+Shift+F8')
        prior = app.status()['manualCapture']['finishedAt']
        shortcut(app, 0x77)
        wait_for(lambda: app.status()['manualCapture']['finishedAt'] != prior)
        assert app.status()['manualCapture']['success']
        app.event(0x02B1, 7)  # Simulated session lock; never lock the user's desktop.
        prior = app.status()['manualCapture']
        shortcut(app, 0x77)
        time.sleep(.3)
        assert app.status()['manualCapture'] == prior and not app.status()['busy']
        app.event(0x02B1, 8)
        checks.append('live key reassignment captures; unavailable desktop rejects shortcut capture')

        app.close()
        assert USER32.RegisterHotKey(None, 100, MODIFIERS, 0x77), 'Shortcut was not released on exit.'
        assert USER32.UnregisterHotKey(None, 100)
        app = Running(folder)
        wait_for(lambda: app.status()['shortcutActive'] == 'Ctrl+Alt+Shift+F8')
        settings = app.status()['settings']; settings['captureShortcut']['enabled'] = False
        app.save(settings)
        wait_for(lambda: app.status()['shortcutActive'] is None)
        # A posted refresh may still be queued after the save response.
        wait_for(lambda: bool(USER32.RegisterHotKey(None, 100, MODIFIERS, 0x77)))
        assert USER32.UnregisterHotKey(None, 100)
        checks.append('shortcut persists across restart; disable and exit release the key')
        report = {'passed': checks, 'folder': str(folder)}
        (folder / 'hotkey-results.json').write_text(json.dumps(report, indent=2), encoding='utf-8')
        print(json.dumps(report, indent=2))
    finally:
        if reserved:
            USER32.UnregisterHotKey(None, RESERVATION)
        USER32.UnregisterHotKey(None, 100)
        app.close()


if __name__ == '__main__':
    main()
