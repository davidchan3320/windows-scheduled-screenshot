#!/usr/bin/env python3
"""Exercise the real app with isolated settings and restartable runtime artifacts."""
import argparse
from datetime import datetime, timedelta
import json
import os
from pathlib import Path
import subprocess
import time
import uuid

parser = argparse.ArgumentParser()
parser.add_argument('executable', type=Path)
parser.add_argument('--capture', action='store_true', help='Requires pre-granted Screen Recording access')
args = parser.parse_args()
case = Path(__file__).resolve().parent / 'artifacts' / (time.strftime('%Y%m%d-%H%M%S') + '-native')
case.mkdir(parents=True)
settings_path = case / 'settings.json'
runtime_path = case / 'runtime-state.json'
command = [str(args.executable.resolve()), '--headless', '--run-for', '180', '--data-directory', str(case)]
process = None

def write(path, value):
    temporary = path.with_suffix('.replacement.json')
    temporary.write_text(json.dumps(value, indent=2))
    os.replace(temporary, path)

def read(path):
    return json.loads(path.read_text())

def events():
    entries = []
    for path in (case / 'logs').glob('*.jsonl'):
        for line in path.read_text().splitlines():
            try:
                entries.append(json.loads(line))
            except json.JSONDecodeError:
                pass
    return entries

def event_count(name):
    return sum(entry.get('eventId') == name for entry in events())

def wait(predicate, message, timeout=12):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if predicate():
            return
        if process is not None and process.poll() is not None:
            raise AssertionError(f'Application exited: {process.returncode}')
        time.sleep(0.1)
    raise AssertionError(message)

def stop():
    global process
    if process is not None:
        process.terminate()
        process.wait(timeout=10)
        process = None

def start():
    global process
    starts = event_count('APP_START')
    process = subprocess.Popen(command)
    wait(lambda: event_count('APP_START') > starts, 'Application did not start')

def apply(settings):
    accepted = event_count('CONFIG_ACCEPTED')
    write(settings_path, settings)
    wait(lambda: event_count('CONFIG_ACCEPTED') > accepted, 'Valid settings were not accepted')

def runtime(task_id):
    return read(runtime_path).get('tasks', {}).get(task_id, {}) if runtime_path.exists() else {}

try:
    start()
    wait(settings_path.exists, 'Default settings missing')
    settings = read(settings_path)
    task_id = str(uuid.uuid4())
    settings.update(paused=True)
    settings['logging']['level'] = 'debug'
    settings['tasks'] = [{
        'id': task_id, 'name': 'Count case', 'enabled': True,
        'capture': {'outputFolder': str(case / 'images'), 'fileNameTemplate': '{timestamp}_{displayIndex}',
                    'imageFormat': 'png', 'jpegQuality': 85, 'includeCursor': False},
        'schedule': {'type': 'interval', 'intervalSeconds': 1,
                     'weekdays': ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun']},
        'stopCondition': {'mode': 'count', 'captureCount': 2}}]
    apply(settings)
    wait(lambda: runtime(task_id).get('countSignature') == 'count:2', 'Count runtime not initialized')
    assert runtime(task_id).get('completedCaptures', 0) == 0

    # Invalid settings retain the last valid configuration, including missing/fractional counts.
    for value in [None, 0, -1, 1.5, 2147483648]:
        invalid = json.loads(json.dumps(settings))
        invalid['tasks'][0]['stopCondition']['captureCount'] = value
        rejected = event_count('CONFIG_REJECTED')
        write(settings_path, invalid)
        wait(lambda: event_count('CONFIG_REJECTED') > rejected, f'Invalid count accepted: {value}')
        assert runtime(task_id).get('countSignature') == 'count:2'
    write(settings_path, settings)
    time.sleep(0.5)

    # Seed previously completed work while stopped to reproduce a restart at any progress point.
    stop()
    state = read(runtime_path)
    state['tasks'][task_id]['completedCaptures'] = 1
    write(runtime_path, state)
    start()
    assert runtime(task_id)['completedCaptures'] == 1, 'Restart reset capture count'
    settings['tasks'][0]['name'] = 'Renamed count case'
    apply(settings)
    assert runtime(task_id)['completedCaptures'] == 1, 'Rename reset count'
    settings['tasks'][0]['schedule']['intervalSeconds'] = 2
    apply(settings)
    assert runtime(task_id)['completedCaptures'] == 1, 'Schedule edit reset count'
    settings['tasks'][0]['stopCondition']['captureCount'] = 3
    apply(settings)
    wait(lambda: runtime(task_id).get('completedCaptures') == 0, 'Changing limit did not reset count')

    # Completed quotas recover even while paused, without another capture.
    stop()
    state = read(runtime_path)
    state['tasks'][task_id]['completedCaptures'] = 3
    write(runtime_path, state)
    start()
    wait(lambda: not read(settings_path)['tasks'][0]['enabled'], 'Exhausted count did not disable task')
    settings['tasks'][0]['enabled'] = True
    apply(settings)
    wait(lambda: runtime(task_id).get('completedCaptures') == 0, 'Re-enable did not start fresh count')

    # One-shot clock lists persist individual slots and disable when all are consumed.
    task = settings['tasks'][0]
    task['schedule'] = {'type': 'fixedOnce', 'times': ['09:00:00', '12:00:00', '17:00:00'],
                        'weekdays': ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun']}
    task['stopCondition'] = {'mode': 'none'}
    apply(settings)
    stop()
    state = read(runtime_path)
    state['tasks'][task_id]['completedFixedTimes'] = ['09:00:00', '12:00:00']
    write(runtime_path, state)
    start()
    assert runtime(task_id)['completedFixedTimes'] == ['09:00:00', '12:00:00'], 'Restart reset clock slots'
    task['name'] = 'Renamed one-shot list'
    apply(settings)
    assert len(runtime(task_id)['completedFixedTimes']) == 2, 'Rename reset clock slots'
    stop()
    state = read(runtime_path)
    state['tasks'][task_id]['completedFixedTimes'].append('17:00:00')
    write(runtime_path, state)
    start()
    wait(lambda: not read(settings_path)['tasks'][0]['enabled'], 'Completed clock list did not disable task')
    task['enabled'] = True
    apply(settings)
    wait(lambda: runtime(task_id).get('completedFixedTimes') == [], 'Re-enable did not reset clock list')

    if args.capture:
        # Real fixed-time execution: each of three listed times must produce one batch, then stop.
        settings['paused'] = False
        task['schedule']['times'] = [(datetime.now() + timedelta(seconds=s)).strftime('%H:%M:%S') for s in (3, 6, 9)]
        batches = event_count('BATCH_COMPLETE')
        apply(settings)
        wait(lambda: not read(settings_path)['tasks'][0]['enabled'], 'Real one-shot clock list did not complete', 20)
        assert event_count('BATCH_COMPLETE') - batches == 3, 'Clock list did not execute exactly three batches'
        time.sleep(2)
        assert event_count('BATCH_COMPLETE') - batches == 3, 'Completed task captured again'
        images = list((case / 'images').rglob('*.png'))
        assert images, 'Clock executions produced no real screenshots'
        for path in images:
            assert path.read_bytes().startswith(b'\x89PNG\r\n\x1a\n'), 'Invalid PNG artifact'

        # Success-based quotas remain available for interval captures.
        task['enabled'] = True
        task['schedule'] = {'type': 'interval', 'intervalSeconds': 2,
                            'weekdays': ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun']}
        task['stopCondition'] = {'mode': 'count', 'captureCount': 2}
        batches = event_count('BATCH_COMPLETE')
        apply(settings)
        wait(lambda: not read(settings_path)['tasks'][0]['enabled'], 'Real capture count did not complete', 20)
        assert event_count('BATCH_COMPLETE') - batches == 2, 'Count did not stop after two batches'
    else:
        assert event_count('BATCH_START') == 0, 'Paused verification unexpectedly captured'
finally:
    stop()
assert event_count('APP_EXIT') > 0, 'Orderly exit missing'
print(f'Native process case passed. Repeatable app artifacts: {case}')
