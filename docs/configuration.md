# Apply a configuration

Use the task manager to import a saved JSON configuration or save individual settings. Changes saved through the manager apply immediately and do not require **Restart application**.

## When edits take effect

- **Task, folder, and shortcut edits:** after each edit, select **Save task**, **Save folder**, or **Save shortcut**. The change applies as soon as saving succeeds; typing or selecting a value alone leaves it as an unsaved draft. No restart is needed.
- **Language and Start at sign-in:** changes save and apply immediately when you change the control.
- **Configuration imports:** changes apply when **Replace configuration** succeeds. Scheduling remains paused until you select **Resume scheduling**.
- **Direct JSON edits:** exit the app, edit and save `data/settings.json`, then reopen the app. Repeat this sequence whenever you edit the file directly.

**Capture now** and **Open captures folder** also save a pending standalone folder edit before using that destination.

## Import a configuration file

1. Open `ScheduledScreenshot.exe`, or select **Open task manager** from its tray menu if it is already running.
2. In **Configuration**, select **Export config** to keep a backup of the current saved tasks and preferences.
3. Select **Import config** and choose your configuration `.json` file. Use a file previously created with **Export config**, or an edited copy of that export.
4. Review the filename and task count in **Replace configuration?**. Select **Cancel** to keep the current configuration, or **Replace configuration** to apply the file.
5. After the import succeeds, review the task list, output folders, schedules, capture shortcut, language, and **Start at sign-in** setting. Import replaces all current tasks and preferences; it does not merge task lists.
6. Enable the tasks you want to run. Imported tasks retain their individual enabled/disabled settings, but scheduling is always paused after import.
7. Select **Resume scheduling** when ready. Confirm that enabled tasks show the expected next capture time.

The file must use `schemaVersion: 2`, contain a `tasks` array, and be smaller than approximately 1 MiB. An empty `tasks` array clears the current task list. Invalid JSON or settings are rejected without replacing the current configuration.

Schedules use Windows local time. Relative output paths, such as `captures`, start beside the executable on the destination computer. Absolute paths keep their specified location; review them when transferring settings between computers. In JSON, write Windows paths with escaped backslashes, for example `"C:\\Screenshots"`.

## Apply changes in the manager

| Change | How to apply it |
| --- | --- |
| Task schedule, output, image settings, or stop condition | Open **Edit**, make changes, then select **Save task**. |
| Destination for standalone captures | Expand **Capture settings**, edit **Save screenshots to**, then select **Save folder**. |
| Global capture shortcut | Expand **Capture settings**, choose the shortcut and its enabled state, then select **Save shortcut**. |
| Language | Choose a value in **Language**; it saves immediately. |
| Start at sign-in | Change **Start at sign-in**; it saves immediately. |
| Pause or resume scheduled captures | Select **Pause scheduling** or **Resume scheduling**. |

The standalone capture folder is separate from each task's output folder. **Capture now** saves a screenshot to the standalone destination. To verify a scheduled task's settings, check its next capture, last result, and **Open folder** after a scheduled run.

## Edit `settings.json` directly

The active configuration is `data/settings.json` beside the executable:

```text
ScheduledScreenshot.exe
data/
  settings.json
  settings.last-good.json
  runtime-state.json
```

1. Select **Exit** from the tray menu. Closing the browser tab leaves the app running.
2. Back up the complete `data` folder if you want to preserve both settings and task progress.
3. Edit `data/settings.json` and save it as UTF-8 JSON. Keep `schemaVersion` at `2` and retain existing task IDs when editing existing tasks. Set `"paused": true` if you want to review the changes before scheduled captures resume.
4. Launch `ScheduledScreenshot.exe` again. The app reads the file at startup; it does not watch the file for changes while running.
5. Review the loaded settings and any error message, then enable tasks and resume scheduling as needed.

Exit **before** editing the file. Editing it while the app is running and then clicking **Restart application** can lose edits if the running app saves its in-memory settings. Restart is unnecessary for imports and changes saved through the manager.

Change **Start at sign-in** through the manager so the app also updates the Windows startup shortcut; editing the JSON value alone does not create or remove that shortcut.

## Settings and progress

**Export config** includes saved tasks and preferences. It does not include screenshots, logs, or capture progress. Importing onto another installation does not restore previous capture counts or completed runs. Existing matching tasks retain progress according to the normal edit rules: changing a schedule or stop condition, or re-enabling a task, can reset the corresponding progress.

To move the complete application state, exit the app and move the executable together with its `data` folder. Move screenshot folders separately if you want to keep their images. After moving the executable, turn **Start at sign-in** off and on again to update the startup shortcut.

## If the configuration does not apply

- **Settings changed in another window:** cancel the import, refresh the manager, and select the file again. For a task edit, keep a copy of your draft before refreshing.
- **Scheduling remains paused:** imports deliberately pause scheduling. Review the tasks, then select **Resume scheduling**. Disabled tasks must also be enabled.
- **Shortcut unavailable:** choose another key combination and select **Save shortcut**; another application may already use the shortcut.
- **Settings rejected at startup:** the app attempts to load `settings.last-good.json` with scheduling paused. Correct the file while the app is closed, or review and save settings through the manager.
- **Runtime progress is invalid:** close the app and restore `data/runtime-state.json` from a backup. Configuration import does not restore runtime progress.
- **A different portable copy opens:** only one copy runs per Windows session. Exit the running copy from its tray menu before launching the copy whose configuration you want to use.

See the [README](../readme.md) for task creation, scheduling behavior, and portable setup.
