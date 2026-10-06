# Run a list of clock times once

In the configuration editor, choose **Fixed local times — run once**, use **Add time** to enter each clock time on its own row, such as `09:00:00`, `12:00:00`, and `17:00:00`, and select weekdays. Use **Remove** to delete a time. Enable the task and save. Each listed clock time executes once, then the app automatically disables the task. No separate capture count is required.

Each time uses its next occurrence on a selected weekday. If you enable the example at 10:00, it runs at 12:00 and 17:00 today, then at 09:00 on the next selected weekday. Completed times persist across restarts and pauses. Re-enable the completed task to run the list again; editing the schedule starts a new list.

```json
"schedule": {
  "type": "fixedOnce",
  "weekdays": ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"],
  "times": ["09:00:00", "12:00:00", "17:00:00"]
}
```

An attempted capture consumes that clock time even if it fails. Busy or unavailable sessions leave the time pending for the next selected weekday. Manual captures do not consume listed times.

## Optional capture count

In the configuration editor, select **Fixed local times**, add execution times on separate rows, such as `09:00:00`, `12:00:00`, and `17:00:00`, and choose the weekdays. Under **End condition**, select **After a number of captures** and enter a positive whole number. Enable the task and save the settings.

For example, a limit of 5 with those three times captures at the next five eligible times, across days if necessary, then automatically disables the task. The same stop mode also works with **Recurring interval**.

```json
"stopCondition": {
  "mode": "count",
  "captureCount": 5
}
```

The count measures successful scheduled executions per task. An execution that writes screenshots for several displays counts once. Manual captures, failed captures, and skipped executions do not consume the count. A capture with at least one saved display image is successful.

Progress persists in `runtime-state.json` across restarts and pause/resume. Changing execution times does not reset the count. Changing the capture limit starts a new count, as does disabling and re-enabling the task. After completion, enable the task again to run another set of captures.

Both Windows and macOS accept counts from 1 to 2,147,483,647. Existing tasks without this stop mode continue to work.

The repeatable native process case is `tests/cases/capture-count/e2e.py`. Run it against a built Mac executable; add `--capture` to verify successful captures with pre-granted Screen Recording permission. The browser case is `tests/cases/capture-count/editor.e2e.cjs` and uses Playwright, optionally located through `PLAYWRIGHT_MODULE`. Both retain application artifacts in the case's `artifacts` directory.
