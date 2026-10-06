using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using System.Threading;
using ScheduledScreenshot.Models;

namespace ScheduledScreenshot.Services
{
    internal sealed class SchedulerService : IDisposable
    {
        private readonly object _sync = new object();
        private readonly ConfigurationService _configuration;
        private readonly RuntimeStateStore _stateStore;
        private readonly CaptureCoordinator _capture;
        private readonly DiagnosticLogger _logger;
        private readonly Timer _timer;
        private AppSettings _settings;
        private RuntimeState _state;
        private bool _captureAllowed = true;
        private bool _disposed;
        private TaskOccurrence _nextOccurrence;
        private DateTimeOffset? _nextDeadline;

        public SchedulerService(
            ConfigurationService configuration,
            RuntimeStateStore stateStore,
            CaptureCoordinator capture,
            DiagnosticLogger logger)
        {
            _configuration = configuration;
            _stateStore = stateStore;
            _capture = capture;
            _logger = logger;
            _settings = configuration.Current;
            _state = stateStore.Snapshot();
            _timer = new Timer(OnTimer, null, Timeout.Infinite, Timeout.Infinite);
            _configuration.SettingsChanged += ApplySettings;
            ReconcileState(DateTimeOffset.UtcNow);
            Reschedule();
        }

        public event Action SchedulerStatusChanged;

        public SchedulerStatus GetStatus()
        {
            lock (_sync)
            {
                return new SchedulerStatus
                {
                    Paused = _settings?.paused ?? true,
                    CaptureAllowed = _captureAllowed,
                    NextTaskName = _nextOccurrence?.Task?.name,
                    NextOccurrenceUtc = _nextOccurrence?.DueUtc,
                    NextDeadlineUtc = _nextDeadline
                };
            }
        }

        public void SetCaptureAllowed(bool allowed, string reason)
        {
            lock (_sync)
            {
                if (_captureAllowed == allowed)
                {
                    return;
                }
                _captureAllowed = allowed;
                _logger.Info(allowed ? "CAPTURE_RESUMED" : "CAPTURE_SUSPENDED",
                    allowed ? "Scheduled capture is available." : "Scheduled capture is temporarily unavailable.",
                    new LogContext { Values = new Dictionary<string, object> { ["reason"] = reason } }, true);
                RescheduleLocked(DateTimeOffset.UtcNow);
            }
            SchedulerStatusChanged?.Invoke();
        }

        public void NotifyClockChanged()
        {
            _logger.Info("CLOCK_CHANGED", "The system clock or timezone changed; schedules were recalculated.", flush: true);
            lock (_sync)
            {
                RescheduleLocked(DateTimeOffset.UtcNow);
            }
            SchedulerStatusChanged?.Invoke();
        }

        public void Dispose()
        {
            lock (_sync)
            {
                if (_disposed) return;
                _disposed = true;
                _timer.Dispose();
            }
            _configuration.SettingsChanged -= ApplySettings;
        }

        private void ApplySettings(AppSettings settings)
        {
            lock (_sync)
            {
                if (_disposed) return;
                _settings = settings;
                ReconcileStateLocked(DateTimeOffset.UtcNow);
                RescheduleLocked(DateTimeOffset.UtcNow);
            }
            SchedulerStatusChanged?.Invoke();
        }

        private void ReconcileState(DateTimeOffset nowUtc)
        {
            lock (_sync)
            {
                ReconcileStateLocked(nowUtc);
            }
        }

        private void ReconcileStateLocked(DateTimeOffset nowUtc)
        {
            var changed = false;
            var enabledIds = new HashSet<string>(
                _settings.tasks.Where(task => task.enabled).Select(task => task.id),
                StringComparer.OrdinalIgnoreCase);
            foreach (var staleId in _state.tasks.Keys.Where(id => !enabledIds.Contains(id)).ToList())
            {
                _state.tasks.Remove(staleId);
                changed = true;
            }

            foreach (var task in _settings.tasks.Where(item => item.enabled))
            {
                if (!_state.tasks.TryGetValue(task.id, out var state))
                {
                    state = new TaskRuntimeState();
                    _state.tasks[task.id] = state;
                    changed = true;
                }

                var scheduleSignature = JsonUtility.Signature(task.schedule);
                if (state.scheduleSignature != scheduleSignature)
                {
                    state.scheduleSignature = scheduleSignature;
                    state.intervalAnchorUtc = nowUtc.ToString("o", CultureInfo.InvariantCulture);
                    state.lastFixedOccurrence = null;
                    state.completedFixedTimes = new List<string>();
                    changed = true;
                }
                else if (state.completedFixedTimes == null)
                {
                    state.completedFixedTimes = new List<string>();
                    changed = true;
                }

                var durationSignature = JsonUtility.Signature(new
                {
                    task.stopCondition.mode,
                    task.stopCondition.endAtLocal,
                    task.stopCondition.durationSeconds
                });
                if (task.stopCondition.mode == "duration")
                {
                    if (state.durationSignature != durationSignature
                        || !DateTimeOffset.TryParse(state.durationDeadlineUtc, out _))
                    {
                        state.durationSignature = durationSignature;
                        state.durationDeadlineUtc = nowUtc.AddSeconds(task.stopCondition.durationSeconds.Value)
                            .ToString("o", CultureInfo.InvariantCulture);
                        changed = true;
                    }
                }
                else if (state.durationDeadlineUtc != null || state.durationSignature != durationSignature)
                {
                    state.durationSignature = durationSignature;
                    state.durationDeadlineUtc = null;
                    changed = true;
                }

                var countSignature = ScheduleCalculator.GetCountSignature(task);
                if (countSignature != null)
                {
                    if (!string.Equals(state.countSignature, countSignature, StringComparison.Ordinal))
                    {
                        state.countSignature = countSignature;
                        state.completedCaptures = 0;
                        changed = true;
                    }
                    else if (state.completedCaptures < 0)
                    {
                        state.completedCaptures = 0;
                        changed = true;
                    }
                }
                else if (state.countSignature != null)
                {
                    state.countSignature = null;
                    changed = true;
                }
            }

            if (changed)
            {
                PersistStateLocked();
            }
        }

        private void Reschedule()
        {
            lock (_sync)
            {
                RescheduleLocked(DateTimeOffset.UtcNow);
            }
            SchedulerStatusChanged?.Invoke();
        }

        private void RescheduleLocked(DateTimeOffset nowUtc)
        {
            if (_disposed) return;
            _timer.Change(Timeout.Infinite, Timeout.Infinite);
            _nextOccurrence = null;
            _nextDeadline = null;

            foreach (var task in _settings.tasks.Where(item => item.enabled))
            {
                if (!_state.tasks.TryGetValue(task.id, out var state))
                {
                    continue;
                }
                if (ScheduleCalculator.IsCountExhausted(task, state)
                    || ScheduleCalculator.IsFixedOnceExhausted(task, state))
                {
                    if (!_nextDeadline.HasValue || nowUtc < _nextDeadline)
                    {
                        _nextDeadline = nowUtc;
                    }
                    continue;
                }
                var deadline = ScheduleCalculator.GetStopUtc(task, state);
                if (deadline.HasValue && (!_nextDeadline.HasValue || deadline < _nextDeadline))
                {
                    _nextDeadline = deadline;
                }
                if (!_settings.paused && _captureAllowed)
                {
                    var occurrence = ScheduleCalculator.GetNextOccurrence(task, state, nowUtc);
                    if (occurrence != null && (_nextOccurrence == null || occurrence.DueUtc < _nextOccurrence.DueUtc))
                    {
                        _nextOccurrence = occurrence;
                    }
                }
            }

            DateTimeOffset? nextEvent = _nextOccurrence?.DueUtc;
            if (_nextDeadline.HasValue && (!nextEvent.HasValue || _nextDeadline < nextEvent))
            {
                nextEvent = _nextDeadline;
            }
            if (!nextEvent.HasValue)
            {
                return;
            }
            var milliseconds = (long)Math.Ceiling((nextEvent.Value - nowUtc).TotalMilliseconds);
            milliseconds = Math.Max(1, Math.Min(milliseconds, int.MaxValue));
            _timer.Change((int)milliseconds, Timeout.Infinite);
            _logger.Debug("NEXT_OCCURRENCE_CALCULATED", "The scheduler timer was armed.",
                new LogContext
                {
                    TaskId = _nextOccurrence?.Task?.id,
                    TaskName = _nextOccurrence?.Task?.name,
                    Values = new Dictionary<string, object>
                    {
                        ["nextEventUtc"] = nextEvent.Value.ToString("o", CultureInfo.InvariantCulture)
                    }
                });
        }

        private void OnTimer(object stateObject)
        {
            try
            {
                ProcessTimer();
            }
            catch (Exception exception)
            {
                _logger.Error("SCHEDULER_ERROR", "The scheduler callback failed; it will be recalculated.", exception);
            }
            finally
            {
                lock (_sync)
                {
                    if (!_disposed)
                    {
                        _settings = _configuration.Current;
                        ReconcileStateLocked(DateTimeOffset.UtcNow);
                        RescheduleLocked(DateTimeOffset.UtcNow);
                    }
                }
                SchedulerStatusChanged?.Invoke();
            }
        }

        private void ProcessTimer()
        {
            List<ScreenshotTaskSettings> expiredTasks;
            HashSet<string> exhaustedFixedOnceIds;
            Dictionary<string, string> expiredRuntimeAnchors;
            List<TaskOccurrence> due;
            var nowUtc = DateTimeOffset.UtcNow;
            lock (_sync)
            {
                if (_disposed) return;
                _timer.Change(Timeout.Infinite, Timeout.Infinite);
                expiredTasks = _settings.tasks.Where(task => task.enabled
                    && _state.tasks.TryGetValue(task.id, out var runtime)
                    && ScheduleCalculator.HasReachedEndCondition(task, runtime, nowUtc))
                    .ToList();
                exhaustedFixedOnceIds = new HashSet<string>(
                    expiredTasks.Where(task => _state.tasks.TryGetValue(task.id, out var runtime)
                        && ScheduleCalculator.IsFixedOnceExhausted(task, runtime))
                        .Select(task => task.id),
                    StringComparer.OrdinalIgnoreCase);
                expiredRuntimeAnchors = expiredTasks.ToDictionary(
                    task => task.id,
                    task => _state.tasks[task.id].intervalAnchorUtc,
                    StringComparer.OrdinalIgnoreCase);

                var expiredIds = new HashSet<string>(expiredTasks.Select(task => task.id), StringComparer.OrdinalIgnoreCase);
                due = new List<TaskOccurrence>();
                if (!_settings.paused && _captureAllowed)
                {
                    foreach (var task in _settings.tasks.Where(item => item.enabled && !expiredIds.Contains(item.id)))
                    {
                        if (!_state.tasks.TryGetValue(task.id, out var runtime)) continue;
                        var occurrence = ScheduleCalculator.GetNextOccurrence(task, runtime, nowUtc.AddMilliseconds(-500));
                        if (occurrence != null && occurrence.DueUtc <= nowUtc.AddMilliseconds(150))
                        {
                            due.Add(occurrence);
                        }
                    }
                }
            }

            foreach (var task in expiredTasks)
            {
                var scheduleSignature = JsonUtility.Signature(task.schedule);
                bool disabled;
                if (exhaustedFixedOnceIds.Contains(task.id))
                {
                    disabled = DisableFixedOnceTaskIfMatching(task.id, scheduleSignature,
                        expiredRuntimeAnchors[task.id]);
                }
                else if (task.stopCondition.mode == "count")
                {
                    disabled = DisableCountTaskIfMatching(task.id, scheduleSignature,
                        ScheduleCalculator.GetCountSignature(task), expiredRuntimeAnchors[task.id]);
                }
                else
                {
                    disabled = _configuration.DisableTask(task.id);
                }
                if (disabled)
                {
                    _logger.Info("TASK_ENDED", "A task reached its configured end condition.", LogContext.ForTask(task), true);
                }
            }

            if (due.Count > 0)
            {
                var captureResult = _capture.TryCapture(due.Select(item => item.Task).ToList(), "scheduled");
                List<TaskOccurrence> completedFixedOnceOccurrences;
                var completedCountOccurrences = UpdateOccurrenceState(
                    due, captureResult, out completedFixedOnceOccurrences);
                foreach (var occurrence in completedCountOccurrences)
                {
                    if (DisableCountTaskIfMatching(occurrence.Task.id, occurrence.ScheduleSignature,
                        occurrence.CountSignature, occurrence.RuntimeAnchorUtc))
                    {
                        _logger.Info("TASK_ENDED", "A task reached its configured end condition.",
                            LogContext.ForTask(occurrence.Task), true);
                    }
                }
                foreach (var occurrence in completedFixedOnceOccurrences)
                {
                    if (DisableFixedOnceTaskIfMatching(occurrence.Task.id, occurrence.ScheduleSignature,
                        occurrence.RuntimeAnchorUtc))
                    {
                        _logger.Info("TASK_ENDED", "A task reached its configured end condition.",
                            LogContext.ForTask(occurrence.Task), true);
                    }
                }
                if (captureResult.Skipped)
                {
                    foreach (var occurrence in due)
                    {
                        _logger.Info("TASK_SKIPPED_BUSY", "A scheduled occurrence was skipped because capture was busy.",
                            LogContext.ForTask(occurrence.Task));
                    }
                }
            }
        }

        private List<TaskOccurrence> UpdateOccurrenceState(
            IEnumerable<TaskOccurrence> occurrences,
            CaptureBatchResult captureResult,
            out List<TaskOccurrence> completedFixedOnceOccurrences)
        {
            var changed = false;
            var completedCountOccurrences = new List<TaskOccurrence>();
            completedFixedOnceOccurrences = new List<TaskOccurrence>();
            var successfulTaskIds = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
            if (captureResult != null)
            {
                successfulTaskIds.UnionWith(captureResult.SuccessfulTaskIds);
            }
            var countedTaskIds = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
            var executedBatch = captureResult != null && !captureResult.Skipped;
            lock (_sync)
            {
                _settings = _configuration.Current;
                ReconcileStateLocked(DateTimeOffset.UtcNow);

                foreach (var occurrence in occurrences)
                {
                    if (!_state.tasks.TryGetValue(occurrence.Task.id, out var state))
                    {
                        continue;
                    }

                    var currentTask = _settings.tasks.FirstOrDefault(item => item.enabled
                        && string.Equals(item.id, occurrence.Task.id, StringComparison.OrdinalIgnoreCase));
                    var runtimeStillMatches = string.Equals(state.intervalAnchorUtc,
                                                  occurrence.RuntimeAnchorUtc, StringComparison.Ordinal)
                        && string.Equals(state.scheduleSignature, occurrence.ScheduleSignature, StringComparison.Ordinal)
                        && currentTask != null;
                    var scheduleStillMatches = runtimeStillMatches
                        && string.Equals(JsonUtility.Signature(currentTask.schedule),
                            occurrence.ScheduleSignature, StringComparison.Ordinal);

                    if (occurrence.FixedOccurrenceKey != null && scheduleStillMatches)
                    {
                        state.lastFixedOccurrence = occurrence.FixedOccurrenceKey;
                        changed = true;
                    }

                    if (executedBatch
                        && occurrence.FixedTimeSlot != null
                        && scheduleStillMatches
                        && currentTask.schedule.type == "fixedOnce"
                        && !state.completedFixedTimes.Contains(occurrence.FixedTimeSlot, StringComparer.Ordinal))
                    {
                        state.completedFixedTimes.Add(occurrence.FixedTimeSlot);
                        changed = true;
                        if (ScheduleCalculator.IsFixedOnceExhausted(currentTask, state))
                        {
                            completedFixedOnceOccurrences.Add(occurrence);
                        }
                    }

                    if (occurrence.CountSignature == null
                        || !successfulTaskIds.Contains(occurrence.Task.id)
                        || !countedTaskIds.Add(occurrence.Task.id)
                        || !string.Equals(state.countSignature, occurrence.CountSignature, StringComparison.Ordinal)
                        || !string.Equals(state.scheduleSignature, occurrence.ScheduleSignature, StringComparison.Ordinal))
                    {
                        continue;
                    }

                    if (!scheduleStillMatches
                        || !string.Equals(ScheduleCalculator.GetCountSignature(currentTask),
                            occurrence.CountSignature, StringComparison.Ordinal))
                    {
                        continue;
                    }

                    if (state.completedCaptures < currentTask.stopCondition.captureCount.Value)
                    {
                        state.completedCaptures++;
                        changed = true;
                    }
                    if (state.completedCaptures >= currentTask.stopCondition.captureCount.Value)
                    {
                        completedCountOccurrences.Add(occurrence);
                    }
                }
                if (changed)
                {
                    PersistStateLocked();
                }
            }
            return completedCountOccurrences;
        }

        private bool DisableFixedOnceTaskIfMatching(
            string taskId,
            string scheduleSignature,
            string runtimeAnchorUtc)
        {
            if (scheduleSignature == null || runtimeAnchorUtc == null)
            {
                return false;
            }
            lock (_sync)
            {
                _settings = _configuration.Current;
                ReconcileStateLocked(DateTimeOffset.UtcNow);
                if (!_state.tasks.TryGetValue(taskId, out var runtime)
                    || !string.Equals(runtime.intervalAnchorUtc, runtimeAnchorUtc, StringComparison.Ordinal)
                    || !string.Equals(runtime.scheduleSignature, scheduleSignature, StringComparison.Ordinal))
                {
                    return false;
                }
                var currentTask = _settings.tasks.FirstOrDefault(item => item.enabled
                    && string.Equals(item.id, taskId, StringComparison.OrdinalIgnoreCase));
                if (currentTask == null || !ScheduleCalculator.IsFixedOnceExhausted(currentTask, runtime))
                {
                    return false;
                }
                return _configuration.Update(settings =>
                {
                    var task = settings.tasks.FirstOrDefault(item => item.enabled
                        && string.Equals(item.id, taskId, StringComparison.OrdinalIgnoreCase));
                    if (task?.schedule?.type != "fixedOnce"
                        || !string.Equals(JsonUtility.Signature(task.schedule),
                            scheduleSignature, StringComparison.Ordinal))
                    {
                        return false;
                    }
                    task.enabled = false;
                    return true;
                });
            }
        }

        private bool DisableCountTaskIfMatching(
            string taskId,
            string scheduleSignature,
            string countSignature,
            string runtimeAnchorUtc)
        {
            if (scheduleSignature == null || countSignature == null || runtimeAnchorUtc == null)
            {
                return false;
            }
            lock (_sync)
            {
                _settings = _configuration.Current;
                ReconcileStateLocked(DateTimeOffset.UtcNow);
                if (!_state.tasks.TryGetValue(taskId, out var runtime)
                    || !string.Equals(runtime.intervalAnchorUtc, runtimeAnchorUtc, StringComparison.Ordinal)
                    || !string.Equals(runtime.scheduleSignature, scheduleSignature, StringComparison.Ordinal)
                    || !string.Equals(runtime.countSignature, countSignature, StringComparison.Ordinal))
                {
                    return false;
                }
                var currentTask = _settings.tasks.FirstOrDefault(item => item.enabled
                    && string.Equals(item.id, taskId, StringComparison.OrdinalIgnoreCase));
                if (currentTask == null || !ScheduleCalculator.IsCountExhausted(currentTask, runtime))
                {
                    return false;
                }
                return _configuration.Update(settings =>
                {
                    var task = settings.tasks.FirstOrDefault(item => item.enabled
                        && string.Equals(item.id, taskId, StringComparison.OrdinalIgnoreCase));
                    if (task == null
                        || !string.Equals(JsonUtility.Signature(task.schedule),
                            scheduleSignature, StringComparison.Ordinal)
                        || !string.Equals(ScheduleCalculator.GetCountSignature(task),
                            countSignature, StringComparison.Ordinal))
                    {
                        return false;
                    }
                    task.enabled = false;
                    return true;
                });
            }
        }

        private void PersistStateLocked()
        {
            var snapshot = JsonUtility.Deserialize<RuntimeState>(JsonUtility.Serialize(_state));
            _stateStore.Mutate(state =>
            {
                state.tasks = snapshot.tasks;
            });
        }
    }

    internal sealed class SchedulerStatus
    {
        public bool Paused { get; set; }
        public bool CaptureAllowed { get; set; }
        public string NextTaskName { get; set; }
        public DateTimeOffset? NextOccurrenceUtc { get; set; }
        public DateTimeOffset? NextDeadlineUtc { get; set; }
    }
}
