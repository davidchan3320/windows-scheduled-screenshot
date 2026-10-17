"""Measure idle resources, then exercise scheduled captures over a chosen duration.
Defaults to 24 hours. Does not open a browser or alter user settings/startup.
Deletes only captures generated inside its unique artifacts directory.
"""
import argparse
import ctypes
from ctypes import wintypes
import json
from pathlib import Path
import shutil
import time
import uuid
from smoke import ROOT, Running, wait_for

KERNEL = ctypes.WinDLL('kernel32', use_last_error=True)
PSAPI = ctypes.WinDLL('psapi', use_last_error=True)
KERNEL.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
KERNEL.OpenProcess.restype = wintypes.HANDLE
KERNEL.CloseHandle.argtypes = [wintypes.HANDLE]
KERNEL.GetProcessHandleCount.argtypes = [wintypes.HANDLE, ctypes.POINTER(wintypes.DWORD)]
KERNEL.GetProcessTimes.argtypes = [wintypes.HANDLE] + [ctypes.POINTER(wintypes.FILETIME)] * 4


class Memory(ctypes.Structure):
    _fields_ = [('cb', wintypes.DWORD), ('faults', wintypes.DWORD)] + [(name, ctypes.c_size_t) for name in ['peak_ws', 'ws', 'peak_pool', 'pool', 'peak_nonpaged', 'nonpaged', 'pagefile', 'peak_pagefile', 'private', 'private_ws']] + [('shared_commit', ctypes.c_ulonglong)]


PSAPI.GetProcessMemoryInfo.argtypes = [wintypes.HANDLE, ctypes.c_void_p, wintypes.DWORD]


def sample(pid):
    handle = KERNEL.OpenProcess(0x0400 | 0x0010, False, pid)
    if not handle:
        raise ctypes.WinError(ctypes.get_last_error())
    try:
        memory = Memory(); memory.cb = ctypes.sizeof(memory)
        if not PSAPI.GetProcessMemoryInfo(handle, ctypes.byref(memory), memory.cb):
            raise ctypes.WinError(ctypes.get_last_error())
        handles = wintypes.DWORD()
        assert KERNEL.GetProcessHandleCount(handle, ctypes.byref(handles))
        times = [wintypes.FILETIME() for _ in range(4)]
        assert KERNEL.GetProcessTimes(handle, *(ctypes.byref(t) for t in times))
        cpu = sum((t.dwHighDateTime << 32) + t.dwLowDateTime for t in times[2:]) / 10_000_000
        return {'elapsed': time.monotonic(), 'privateWorkingSetBytes': memory.private_ws, 'privateBytes': memory.private, 'workingSetBytes': memory.ws, 'handles': handles.value, 'cpuSeconds': cpu}
    finally:
        KERNEL.CloseHandle(handle)


def main():
    p = argparse.ArgumentParser()
    p.add_argument('--minutes', type=float, default=1440)
    p.add_argument('--idle-seconds', type=int, default=60)
    p.add_argument('--interval', type=int, default=60)
    p.add_argument('--exe', type=Path, default=ROOT / 'target/x86_64-pc-windows-msvc/release/ScheduledScreenshot.exe')
    args = p.parse_args()
    folder = ROOT / 'artifacts' / ('soak-' + uuid.uuid4().hex[:8]); folder.mkdir(parents=True)
    shutil.copy2(args.exe, folder / 'ScheduledScreenshot.exe')
    app = Running(folder)
    report = {'requestedMinutes': args.minutes, 'complete': False, 'samples': []}
    report_path = folder / 'soak-results.json'
    report_path.write_text(json.dumps(report, indent=2), encoding='utf-8')
    print(json.dumps({'started': True, 'report': str(report_path), 'applicationPid': app.process.pid}), flush=True)
    try:
        time.sleep(3)  # startup settling, excluded from idle measurement
        first = sample(app.process.pid)
        time.sleep(args.idle_seconds)
        last = sample(app.process.pid)
        idle_cpu = 100 * (last['cpuSeconds'] - first['cpuSeconds']) / (last['elapsed'] - first['elapsed'])
        report['idle'] = {'seconds': last['elapsed'] - first['elapsed'], 'cpuPercentOneCore': idle_cpu, 'privateWorkingSetBytes': last['privateWorkingSetBytes'], 'handles': last['handles'], 'targetsMet': idle_cpu < .05 and last['privateWorkingSetBytes'] <= 40 * 1024 * 1024}
        print(json.dumps({'report': str(report_path), 'idle': report['idle']}), flush=True)
        status = app.status(); task = status['defaultTask']; task.update(id=str(uuid.uuid4()), name='Soak capture', enabled=True)
        task['capture'].update(outputFolder='soak-captures', dateSubfolders=False)
        task['schedule']['intervalSeconds'] = args.interval
        status['settings']['tasks'] = [task]; app.save(status['settings'])
        start = time.monotonic(); end = start + args.minutes * 60; successful = 0
        while time.monotonic() < end:
            time.sleep(min(5, max(0, end-time.monotonic())))
            assert app.process.poll() is None, 'Scheduler exited unexpectedly'
            current = app.status()
            progress = current['tasks'][0]['progress']
            if progress and progress['completedCaptures'] > successful:
                successful = progress['completedCaptures']
                for saved in progress['lastFiles']:
                    path = Path(saved).resolve()
                    assert path.is_relative_to(folder.resolve()), 'Capture escaped test directory'
                    assert path.stat().st_size > 0
                    path.unlink()
            report['samples'].append(dict(sample(app.process.pid), successfulCaptures=successful, captureAllowed=current['captureAllowed'], busy=current['busy'], lastResult=progress and progress['lastResult']))
            report['successfulCaptures'] = successful
            report['elapsedSeconds'] = time.monotonic()-start
            report_path.write_text(json.dumps(report, indent=2), encoding='utf-8')
        settings = app.status()['settings']; settings['paused'] = True; app.save(settings)
        wait_for(lambda: not app.status()['busy'])
        time.sleep(3)
        report['successfulCaptures'] = app.status()['tasks'][0]['progress']['completedCaptures']
        capture_root = folder / 'soak-captures'
        for path in capture_root.glob('*'):
            assert path.resolve().is_relative_to(folder.resolve())
            if path.is_file():
                path.unlink()
        report['final'] = sample(app.process.pid)
        report['complete'] = True
        report['full24Hours'] = report['elapsedSeconds'] >= 86400
        report_path.write_text(json.dumps(report, indent=2), encoding='utf-8')
        print(json.dumps({'report': str(report_path), 'successfulCaptures': successful, 'full24Hours': report['full24Hours'], 'final': report['final']}), flush=True)
    finally:
        app.close()


if __name__ == '__main__':
    main()
