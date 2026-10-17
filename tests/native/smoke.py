"""Exercise the packaged Windows EXE in an isolated, portable directory.
No developer runtime is used by the EXE. Python/Playwright are test drivers only.
"""
import argparse
import ctypes
from ctypes import wintypes
import datetime as dt
import json
import os
from pathlib import Path
import shutil
import subprocess
import time
import urllib.error
import urllib.request
import uuid

ROOT = Path(__file__).resolve().parents[2]
USER32 = ctypes.WinDLL('user32', use_last_error=True)
USER32.PostMessageW.argtypes = [wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM]
USER32.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
USER32.GetWindowTextW.argtypes = [wintypes.HWND, wintypes.LPWSTR, ctypes.c_int]
CALLBACK = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
USER32.EnumWindows.argtypes = [CALLBACK, wintypes.LPARAM]


def shortcut_details(path):
    # WScript.Shell can round-trip TargetPath through the ANSI code page.
    # Inspect non-ASCII paths with the Unicode shell interface instead.
    ole = ctypes.OleDLL('ole32')
    class Guid(ctypes.Structure):
        _fields_ = [('bytes', ctypes.c_ubyte * 16)]
        def __init__(self, text):
            super().__init__((ctypes.c_ubyte * 16).from_buffer_copy(uuid.UUID(text).bytes_le))
    def method(pointer, index, *types):
        vtable = ctypes.cast(pointer, ctypes.POINTER(ctypes.POINTER(ctypes.c_void_p))).contents
        return ctypes.WINFUNCTYPE(ctypes.c_long, ctypes.c_void_p, *types)(vtable[index])
    link = ctypes.c_void_p(); persist = ctypes.c_void_p()
    ole.CoInitializeEx(None, 2)
    try:
        ole.CoCreateInstance(ctypes.byref(Guid('00021401-0000-0000-C000-000000000046')), None, 1, ctypes.byref(Guid('000214F9-0000-0000-C000-000000000046')), ctypes.byref(link))
        assert method(link, 0, ctypes.POINTER(Guid), ctypes.POINTER(ctypes.c_void_p))(link, ctypes.byref(Guid('0000010b-0000-0000-C000-000000000046')), ctypes.byref(persist)) == 0
        assert method(persist, 5, wintypes.LPCWSTR, wintypes.DWORD)(persist, str(path), 0) == 0
        target = ctypes.create_unicode_buffer(32768); arguments = ctypes.create_unicode_buffer(1024)
        assert method(link, 3, wintypes.LPWSTR, ctypes.c_int, ctypes.c_void_p, wintypes.DWORD)(link, target, len(target), None, 4) == 0
        assert method(link, 10, wintypes.LPWSTR, ctypes.c_int)(link, arguments, len(arguments)) == 0
        return {'target': target.value, 'arguments': arguments.value}
    finally:
        if persist.value: method(persist, 2)(persist)
        if link.value: method(link, 2)(link)
        ole.CoUninitialize()


def windows_for(pid, title):
    found = []
    @CALLBACK
    def visit(hwnd, _):
        process = wintypes.DWORD()
        USER32.GetWindowThreadProcessId(hwnd, ctypes.byref(process))
        text = ctypes.create_unicode_buffer(512)
        USER32.GetWindowTextW(hwnd, text, len(text))
        if process.value == pid and text.value == title:
            found.append(hwnd)
        return True
    USER32.EnumWindows(visit, 0)
    return found


def wait_for(fn, timeout=35):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        value = fn()
        if value:
            return value
        time.sleep(.15)
    raise AssertionError('Timed out waiting for expected state')


class Running:
    def __init__(self, folder):
        self.folder = folder
        self.log = open(folder / 'stdout.txt', 'w', encoding='utf-8')
        self.process = subprocess.Popen([str(folder / 'ScheduledScreenshot.exe'), '--background', '--print-url'], stdout=self.log, stderr=self.log, creationflags=subprocess.CREATE_NO_WINDOW)
        self.url = wait_for(lambda: (folder / 'stdout.txt').read_text(encoding='utf-8').strip() or None)
        self.origin, self.token = self.url.split('/#token=')
        self.window = wait_for(lambda: windows_for(self.process.pid, 'Scheduled Screenshot'))[0]

    def request(self, route, value=None, token=None, origin=None):
        data = json.dumps(value or {}).encode()
        request = urllib.request.Request(self.origin + '/api/' + route, data=data, headers={'Content-Type': 'application/json', 'X-App-Token': token if token is not None else self.token, 'Origin': origin or self.origin})
        with urllib.request.urlopen(request, timeout=40) as response:
            return json.load(response)

    def status(self):
        return self.request('status')

    def save(self, settings):
        return self.request('save', {'settings': settings, 'revision': settings['revision']})

    def event(self, message, value=0):
        assert USER32.PostMessageW(self.window, message, value, 0)
        time.sleep(.2)

    def close(self):
        if self.process.poll() is None:
            USER32.PostMessageW(self.window, 0x0010, 0, 0)
            self.process.wait(timeout=40)
        self.log.close()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--exe', type=Path, default=ROOT / 'target/x86_64-pc-windows-msvc/release/ScheduledScreenshot.exe')
    parser.add_argument('--browser', action='store_true')
    args = parser.parse_args()
    folder = ROOT / 'artifacts' / ('portable smoke 截圖 ' + uuid.uuid4().hex[:8])
    folder.mkdir(parents=True)
    shutil.copy2(args.exe, folder / 'ScheduledScreenshot.exe')
    app = Running(folder)
    checks = []
    try:
        # A copied executable must reuse the running app, without creating data.
        other = folder / 'another portable copy'
        other.mkdir()
        shutil.copy2(args.exe, other / 'ScheduledScreenshot.exe')
        for executable in [folder / 'ScheduledScreenshot.exe', other / 'ScheduledScreenshot.exe']:
            duplicate = subprocess.run([str(executable), '--background', '--print-url'],
                                       capture_output=True, timeout=15,
                                       creationflags=subprocess.CREATE_NO_WINDOW)
            assert duplicate.returncode == 0 and not duplicate.stdout, duplicate
            assert app.process.poll() is None
            assert app.status()['settings']['schemaVersion'] == 2
        assert not (other / 'data').exists()
        checks.append('one instance across portable folders; duplicate exits without loading settings')
        status = app.status()
        assert status['captureAllowed'], 'Run capture smoke tests in an unlocked interactive Windows session.'
        assert status['settings']['schemaVersion'] == 2
        for kwargs in [{'token': 'wrong'}, {'origin': 'https://untrusted.example'}]:
            try:
                app.request('status', **kwargs)
                raise AssertionError('Unauthorized request accepted')
            except urllib.error.HTTPError as error:
                assert error.code == 403
        checks.append('loopback token/origin rejection')
        # Standalone manual capture must work with zero tasks and while paused.
        settings = app.status()['settings']; settings['paused'] = True
        app.save(settings)
        blocked_capture_folder = folder / 'captures'
        blocked_capture_folder.write_text('This is a file, not a directory.')
        app.request('capture')
        wait_for(lambda: not app.status()['busy'])
        failed_manual = app.status()
        assert failed_manual['manualCapture']['success'] is False
        assert failed_manual['tasks'] == [] and failed_manual['settings']['tasks'] == []
        assert blocked_capture_folder.is_file()
        blocked_capture_folder.unlink()
        app.request('capture')
        wait_for(lambda: not app.status()['busy'])
        manual = app.status()
        assert manual['manualCapture']['success'] is True, manual
        assert not manual['manualCaptureRunning']
        assert manual['settings']['paused'] and manual['tasks'] == []
        assert manual['manualCapture']['files']
        for filename in manual['manualCapture']['files']:
            image = Path(filename)
            assert image.is_relative_to(folder / 'captures') and image.suffix == '.jpg'
            assert image.parent.name == dt.date.today().isoformat()
            assert image.read_bytes().startswith(b'\xff\xd8')
        settings = manual['settings']; settings['paused'] = False
        app.save(settings)
        checks.append('standalone manual JPEG capture with zero tasks while paused; output failure and recovery')
        startup = Path(os.environ['APPDATA']) / 'Microsoft/Windows/Start Menu/Programs/Startup/ScheduledScreenshot.lnk'
        previous = startup.read_bytes() if startup.exists() else None
        try:
            settings = app.status()['settings']; settings['startAtSignIn'] = True
            app.save(settings)
            assert startup.exists()
            shortcut = shortcut_details(startup)
            assert Path(shortcut['target']).samefile(folder / 'ScheduledScreenshot.exe'), shortcut
            assert shortcut['arguments'] == '--background'
            settings = app.status()['settings']; settings['startAtSignIn'] = False
            app.save(settings)
            assert not startup.exists()
        finally:
            if previous is not None:
                startup.write_bytes(previous)
            elif startup.exists():
                startup.unlink()
        checks.append('native Startup shortcut creation, target/arguments and removal (previous shortcut restored)')
        if args.browser:
            env = dict(os.environ, SCHEDULER_URL=app.url, SMOKE_OUTPUT=str(folder))
            subprocess.run(['node', str(ROOT / 'tests/browser/suite.cjs')], env=env, check=True)
            checks.append('browser CRUD, preview, cancellation contract, stale edits, reopen, no external assets')
        status = app.status()
        tasks = []
        for name, fmt in [('JPEG', 'jpeg'), ('PNG', 'png'), ('Output failure', 'jpeg')]:
            task = json.loads(json.dumps(status['defaultTask']))
            task.update(id=str(uuid.uuid4()), name=name, enabled=True)
            task['capture'].update(outputFolder=name, imageFormat=fmt, includeCursor=True)
            task['schedule']['intervalSeconds'] = 1
            task['stopCondition'].update(mode='count', captureCount=1)
            tasks.append(task)
        (folder / 'Output failure').write_text('This is a file, not a directory.')
        status['settings'].update(tasks=tasks, paused=False)
        app.save(status['settings'])
        wait_for(lambda: all(not t['enabled'] for t in app.status()['settings']['tasks'][:2]))
        current = app.status()
        for name, suffix, magic in [('JPEG', '.jpg', b'\xff\xd8'), ('PNG', '.png', b'\x89PNG')]:
            files = list((folder / name).rglob('*' + suffix))
            assert files
            for file in files:
                assert file.read_bytes().startswith(magic)
            p = next(s['progress'] for s in current['tasks'] if s['id'] == next(t['id'] for t in tasks if t['name'] == name))
            assert p['completedCaptures'] == 1
            assert len(p['lastFiles']) == len(files)
        bad = next(s['progress'] for s in current['tasks'] if s['id'] == tasks[2]['id'])
        assert bad['completedCaptures'] == 0
        checks.append('JPEG/PNG per-monitor capture, cursor, exact scheduled counts, isolated output failure')
        # Configure one-time and legacy fixedOnce occurrences together.
        at = (dt.datetime.now() + dt.timedelta(seconds=4)).replace(microsecond=0)
        once = json.loads(json.dumps(current['defaultTask']))
        once.update(id=str(uuid.uuid4()), name='Once', enabled=True)
        once['schedule'].update(type='once', atLocal=at.isoformat())
        legacy = json.loads(json.dumps(once))
        legacy.update(id=str(uuid.uuid4()), name='Legacy once')
        legacy['schedule'].update(type='fixedOnce', times=[at.strftime('%H:%M:%S')])
        current['settings']['tasks'] = [once, legacy]
        app.save(current['settings'])
        wait_for(lambda: all(not t['enabled'] for t in app.status()['settings']['tasks']))
        current = app.status()
        assert current['tasks'][0]['progress']['onceResult'] == 'completed', current
        assert current['tasks'][1]['progress']['completedFixedTimes'] == legacy['schedule']['times'], current
        checks.append('simultaneous dated once and legacy fixedOnce complete without retries')
        # Manual capture ignores saved task format/path and preserves all task progress.
        settings = current['settings']; settings['paused'] = True
        settings['tasks'][0]['capture'].update(outputFolder='Output failure', imageFormat='png')
        app.save(settings)
        before = app.status()
        app.request('capture')
        wait_for(lambda: not app.status()['busy'])
        after = app.status()
        assert after['manualCapture']['success'] is True, after
        assert after['tasks'] == before['tasks']
        assert after['settings'] == before['settings']
        assert all(Path(name).suffix == '.jpg' and Path(name).is_relative_to(folder / 'captures') for name in after['manualCapture']['files'])
        try:
            app.request('capture', {'id': once['id']})
            raise AssertionError('Task-dependent capture request accepted')
        except urllib.error.HTTPError as error:
            assert error.code == 400
        checks.append('manual capture ignores task settings and preserves task progress; task IDs rejected')
        # Simulate notifications on this process only; do not lock the user's session.
        app.event(0x02B1, 7)  # WM_WTSSESSION_CHANGE, WTS_SESSION_LOCK
        assert not app.status()['captureAllowed']
        try:
            app.request('capture')
            raise AssertionError('Manual capture accepted on locked desktop')
        except urllib.error.HTTPError as error:
            assert error.code == 400
        missed = json.loads(json.dumps(once))
        missed.update(id=str(uuid.uuid4()), name='Missed while locked')
        missed['schedule']['atLocal'] = (dt.datetime.now() + dt.timedelta(seconds=2)).replace(microsecond=0).isoformat()
        settings = app.status()['settings']; settings['tasks'] = [missed]
        app.save(settings)
        wait_for(lambda: not app.status()['settings']['tasks'][0]['enabled'])
        assert app.status()['tasks'][0]['progress']['onceResult'] == 'missed'
        app.event(0x02B1, 8)  # unlock
        assert app.status()['captureAllowed']
        app.event(0x001E)  # clock changed, recompute only
        assert app.status()['tasks'][0]['progress']['onceResult'] == 'missed'
        checks.append('simulated lock/unlock and clock notifications skip missed once')
        # Cancel the real native picker in this test process only.
        import concurrent.futures
        with concurrent.futures.ThreadPoolExecutor() as pool:
            future = pool.submit(app.request, 'folder')
            picker = wait_for(lambda: windows_for(app.process.pid, 'Choose screenshot folder'))[0]
            USER32.PostMessageW(picker, 0x0010, 0, 0)
            assert future.result(timeout=10)['path'] is None
        checks.append('real native folder-picker cancellation')
        app.close()
        moved = folder.with_name(folder.name + ' moved')
        folder.rename(moved)
        folder = moved
        app = Running(folder)
        assert app.status()['tasks'][0]['progress']['onceResult'] == 'missed'
        assert app.status()['applicationDirectory'] == str(folder)
        checks.append('move executable with data; progress survives restart')
        # Change the standalone save path without modifying any saved task.
        before = app.status()
        settings = before['settings']; settings['manualOutputFolder'] = 'manual shots/\u622a\u5716'
        app.save(settings)
        before = app.status()
        app.request('capture')
        wait_for(lambda: not app.status()['busy'])
        after = app.status()
        assert after['manualCapture']['success'], after
        assert after['tasks'] == before['tasks'] and after['settings'] == before['settings']
        destination = folder / 'manual shots/\u622a\u5716'
        assert all(Path(name).is_relative_to(destination) and Path(name).is_file() for name in after['manualCapture']['files'])
        app.close()
        app = Running(folder)
        assert app.status()['settings']['manualOutputFolder'] == 'manual shots/\u622a\u5716'
        assert Path(app.status()['manualOutputFolder']) == destination
        settings = app.status()['settings']; settings['manualOutputFolder'] = str(folder / 'absolute output')
        app.save(settings)
        app.request('capture')
        wait_for(lambda: not app.status()['busy'])
        after = app.status()
        assert after['manualCapture']['success'], after
        assert all(Path(name).is_relative_to(folder / 'absolute output') for name in after['manualCapture']['files'])
        checks.append('custom relative Unicode and absolute standalone folders; persisted across restart; task progress unchanged')

        report = {'passed': checks, 'folder': str(folder), 'os': os.name, 'note': 'Notifications were simulated. Real lock/sleep, display/DPI matrix, clean VMs and 24-hour soak are separate release gates.'}
        (folder / 'smoke-results.json').write_text(json.dumps(report, indent=2), encoding='utf-8')
        print(json.dumps(report, indent=2))
    finally:
        app.close()


if __name__ == '__main__':
    main()
