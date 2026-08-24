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
                    changed = true;
                }

                var durationSignature = JsonUtility.Signature(task.stopCondition);
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
            List<TaskOccurrence> due;
            var nowUtc = DateTimeOffset.UtcNow;
            lock (_sync)
            {
                if (_disposed) return;
                _timer.Change(Timeout.Infinite, Timeout.Infinite);
                expiredTasks = _settings.tasks.Where(task => task.enabled
                    && _state.tasks.TryGetValue(task.id, out var runtime)
                    && ScheduleCalculator.GetStopUtc(task, runtime).HasValue
                    && ScheduleCalculator.GetStopUtc(task, runtime).Value <= nowUtc)
                    .ToList();

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
                _logger.Info("TASK_ENDED", "A task reached its configured end condition.", LogContext.ForTask(task), true);
                _configuration.DisableTask(task.id);
            }

            if (due.Count > 0)
            {
                var captureResult = _capture.TryCapture(due.Select(item => item.Task).ToList(), "scheduled");
                UpdateOccurrenceState(due);
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

        private void UpdateOccurrenceState(IEnumerable<TaskOccurrence> occurrences)
        {
            var changed = false;
            lock (_sync)
            {
                foreach (var occurrence in occurrences.Where(item => item.FixedOccurrenceKey != null))
                {
                    if (_state.tasks.TryGetValue(occurrence.Task.id, out var state))
                    {
                        state.lastFixedOccurrence = occurrence.FixedOccurrenceKey;
                        changed = true;
                    }
                }
                if (changed)
                {
                    PersistStateLocked();
                }
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
