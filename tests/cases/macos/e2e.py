#!/usr/bin/env python3
"""Run the native executable through live configuration changes; retain real app artifacts."""
import argparse
from datetime import datetime, timedelta
import json
import os
from pathlib import Path
import subprocess
import time
import uuid

parser = argparse.ArgumentParser()
parser.add_argument("executable", type=Path)
parser.add_argument("--capture", action="store_true", help="Requires existing Screen Recording permission")
args = parser.parse_args()
executable = args.executable.resolve()
case = Path(__file__).resolve().parent / "artifacts" / time.strftime("%Y%m%d-%H%M%S")
case.mkdir(parents=True, exist_ok=False)
settings_path = case / "settings.json"

def write_settings(settings):
    temporary = case / "replacement.json"
    temporary.write_text(json.dumps(settings, indent=2))
    os.replace(temporary, settings_path)

def events():
    entries = []
    for path in (case / "logs").glob("*.jsonl"):
        for line in path.read_text().splitlines():
            try:
                entries.append(json.loads(line))
            except json.JSONDecodeError:
                pass  # An actively written line may be incomplete.
    return entries

def wait_until(predicate, message, timeout=10):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if predicate():
            return
        time.sleep(0.1)
    raise AssertionError(message)

def has_event(event):
    return any(entry.get("eventId") == event for entry in events())

command = [str(executable), "--headless", "--run-for", "60", "--data-directory", str(case)]
process = subprocess.Popen(command)
try:
    wait_until(lambda: settings_path.exists() and has_event("APP_START"), "App did not create default config/start")
    settings = json.loads(settings_path.read_text())
    assert settings["schemaVersion"] == 1
    assert not any(task["enabled"] for task in settings["tasks"])
    duplicate = subprocess.run([str(executable), "--headless", "--run-for", "1", "--data-directory", str(case)], capture_output=True, timeout=5)
    assert duplicate.returncode != 0, "Second instance acquired the same data directory"

    task_id = str(uuid.uuid4())
    settings["logging"]["level"] = "debug"
    settings["customExtension"] = {"keep": True}
    settings["tasks"] = [{"id": task_id, "name": "E2E capture", "enabled": True,
        "customTaskExtension": 7,
        "capture": {"outputFolder": str(case / "images"), "fileNameTemplate": "{timestamp}_{displayIndex}",
                    "imageFormat": "png", "jpegQuality": 85, "includeCursor": False},
        "schedule": {"type": "interval", "intervalSeconds": 1,
                     "weekdays": ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]},
        "stopCondition": {"mode": "duration", "durationSeconds": 3}}]
    if not args.capture:
        # The headless host never prompts for permissions. A stopped session still expires tasks.
        settings["paused"] = True
    write_settings(settings)
    wait_until(lambda: (case / "runtime-state.json").exists() and task_id.lower() in
               json.loads((case / "runtime-state.json").read_text()).get("tasks", {}),
               "Config replacement did not create task runtime state")
    wait_until(lambda: not json.loads(settings_path.read_text())["tasks"][0]["enabled"], "Duration did not expire")
    persisted = json.loads(settings_path.read_text())
    assert persisted["customExtension"] == {"keep": True}, "Unknown app field lost"
    assert persisted["tasks"][0]["customTaskExtension"] == 7, "Unknown task field lost"
    assert has_event("TASK_ENDED"), "Duration end not logged"

    # Exact deadlines shared by several tasks must be disabled in one valid write.
    simultaneous = json.loads(json.dumps(persisted))
    simultaneous["paused"] = True
    end_at = (datetime.now() + timedelta(seconds=4)).strftime("%Y-%m-%dT%H:%M:%S")
    first = simultaneous["tasks"][0]
    first["enabled"] = True
    first["stopCondition"] = {"mode": "at", "endAtLocal": end_at}
    second = json.loads(json.dumps(first))
    second.update(id=str(uuid.uuid4()), name="Second exact deadline")
    simultaneous["tasks"].append(second)
    write_settings(simultaneous)
    wait_until(lambda: not any(task["enabled"] for task in json.loads(settings_path.read_text())["tasks"]),
               "Simultaneous exact deadlines did not end atomically")

    # A restart must retain the original duration deadline rather than starting it again.
    restarted = json.loads(json.dumps(persisted))
    restarted["paused"] = True
    restarted["tasks"][0]["enabled"] = True
    restarted["tasks"][0]["stopCondition"] = {"mode": "duration", "durationSeconds": 6}
    write_settings(restarted)
    runtime_path = case / "runtime-state.json"
    wait_until(lambda: json.loads(runtime_path.read_text()).get("tasks", {}).get(task_id, {}).get("durationDeadlineUtc"),
               "Duration re-enable did not create a deadline")
    deadline = json.loads(runtime_path.read_text())["tasks"][task_id]["durationDeadlineUtc"]
    process.terminate()
    process.wait(timeout=10)
    starts = sum(entry.get("eventId") == "APP_START" for entry in events())
    process = subprocess.Popen(command)
    wait_until(lambda: sum(entry.get("eventId") == "APP_START" for entry in events()) > starts,
               "Restart did not start")
    assert json.loads(runtime_path.read_text())["tasks"][task_id]["durationDeadlineUtc"] == deadline, "Restart reset duration"
    wait_until(lambda: not json.loads(settings_path.read_text())["tasks"][0]["enabled"], "Restarted duration did not expire")

    settings_path.write_text('{"schemaVersion": 999, "tasks": []}')
    wait_until(lambda: has_event("CONFIG_REJECTED"), "Invalid configuration was not rejected")
    accepted = sum(entry.get("eventId") == "CONFIG_ACCEPTED" for entry in events())
    write_settings(persisted)
    # Use a real semantic change to verify recovery from invalid configuration.
    persisted["paused"] = not persisted["paused"]
    write_settings(persisted)
    wait_until(lambda: sum(entry.get("eventId") == "CONFIG_ACCEPTED" for entry in events()) > accepted,
               "Watcher did not recover after invalid atomic replacement")
    if args.capture:
        images = list((case / "images").rglob("*.png"))
        assert images, "Permission-granted capture did not produce images"
        for path in images:
            assert path.read_bytes().startswith(b"\x89PNG\r\n\x1a\n"), "Invalid PNG"
            subprocess.run(["sips", "-g", "pixelWidth", "-g", "pixelHeight", str(path)], check=True)
finally:
    process.terminate()
    process.wait(timeout=10)
assert has_event("APP_EXIT"), "Orderly termination was not logged"
print(f"Native process E2E passed. Repeatable application artifacts: {case}")
