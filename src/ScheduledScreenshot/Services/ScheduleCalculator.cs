using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using ScheduledScreenshot.Models;

namespace ScheduledScreenshot.Services
{
    internal static class ScheduleCalculator
    {
        public static TaskOccurrence GetNextOccurrence(
            ScreenshotTaskSettings task,
            TaskRuntimeState state,
            DateTimeOffset nowUtc)
        {
            if (task == null || !task.enabled || task.schedule == null || state == null)
            {
                return null;
            }
            if (IsCountExhausted(task, state) || IsFixedOnceExhausted(task, state))
            {
                return null;
            }

            var stopUtc = GetStopUtc(task, state);
            TaskOccurrence occurrence;
            if (task.schedule.type == "fixed" || task.schedule.type == "fixedOnce")
            {
                occurrence = GetNextFixed(task, state, nowUtc);
            }
            else
            {
                occurrence = GetNextInterval(task, state, nowUtc);
            }

            if (occurrence != null && stopUtc.HasValue && occurrence.DueUtc >= stopUtc.Value)
            {
                return null;
            }
            if (occurrence != null)
            {
                occurrence.ScheduleSignature = state.scheduleSignature;
                occurrence.RuntimeAnchorUtc = state.intervalAnchorUtc;
                occurrence.CountSignature = GetCountSignature(task);
            }
            return occurrence;
        }

        public static string GetCountSignature(ScreenshotTaskSettings task)
        {
            if (task?.stopCondition?.mode != "count" || !task.stopCondition.captureCount.HasValue)
            {
                return null;
            }
            return "count:" + task.stopCondition.captureCount.Value.ToString(CultureInfo.InvariantCulture);
        }

        public static bool IsCountExhausted(ScreenshotTaskSettings task, TaskRuntimeState state)
        {
            var signature = GetCountSignature(task);
            return signature != null
                   && state != null
                   && string.Equals(state.countSignature, signature, StringComparison.Ordinal)
                   && state.completedCaptures >= task.stopCondition.captureCount.Value;
        }

        public static bool IsFixedOnceExhausted(ScreenshotTaskSettings task, TaskRuntimeState state)
        {
            if (task?.schedule?.type != "fixedOnce" || task.schedule.times == null || state == null)
            {
                return false;
            }
            var completedTimes = state.completedFixedTimes ?? new List<string>();
            return task.schedule.times.All(time => completedTimes.Contains(time, StringComparer.Ordinal));
        }

        public static bool HasReachedEndCondition(
            ScreenshotTaskSettings task,
            TaskRuntimeState state,
            DateTimeOffset nowUtc)
        {
            if (IsCountExhausted(task, state) || IsFixedOnceExhausted(task, state))
            {
                return true;
            }
            var stopUtc = GetStopUtc(task, state);
            return stopUtc.HasValue && stopUtc.Value <= nowUtc;
        }

        public static DateTimeOffset? GetStopUtc(ScreenshotTaskSettings task, TaskRuntimeState state)
        {
            if (task?.stopCondition == null)
            {
                return null;
            }
            if (task.stopCondition.mode == "duration"
                && DateTimeOffset.TryParse(state?.durationDeadlineUtc, CultureInfo.InvariantCulture,
                    DateTimeStyles.RoundtripKind, out var durationDeadline))
            {
                return durationDeadline.ToUniversalTime();
            }
            if (task.stopCondition.mode == "at"
                && SettingsValidator.TryParseLocalDateTime(task.stopCondition.endAtLocal, out var localEnd))
            {
                return ConvertLocalToUtc(localEnd);
            }
            return null;
        }

        public static string DayToken(DayOfWeek day)
        {
            switch (day)
            {
                case DayOfWeek.Monday: return "Mon";
                case DayOfWeek.Tuesday: return "Tue";
                case DayOfWeek.Wednesday: return "Wed";
                case DayOfWeek.Thursday: return "Thu";
                case DayOfWeek.Friday: return "Fri";
                case DayOfWeek.Saturday: return "Sat";
                default: return "Sun";
            }
        }

        private static TaskOccurrence GetNextInterval(
            ScreenshotTaskSettings task,
            TaskRuntimeState state,
            DateTimeOffset nowUtc)
        {
            if (!DateTimeOffset.TryParse(state.intervalAnchorUtc, CultureInfo.InvariantCulture,
                DateTimeStyles.RoundtripKind, out var anchorUtc))
            {
                return null;
            }
            anchorUtc = anchorUtc.ToUniversalTime();
            var nowLocal = TimeZoneInfo.ConvertTime(nowUtc, TimeZoneInfo.Local);
            var interval = task.schedule.intervalSeconds;

            for (var offset = 0; offset <= 8; offset++)
            {
                var localDate = nowLocal.Date.AddDays(offset);
                if (!task.schedule.weekdays.Contains(DayToken(localDate.DayOfWeek)))
                {
                    continue;
                }

                var startTime = TimeSpan.Zero;
                var endTime = TimeSpan.FromDays(1);
                if (task.schedule.activeHoursEnabled)
                {
                    SettingsValidator.TryParseClock(task.schedule.activeStart, out startTime);
                    SettingsValidator.TryParseClock(task.schedule.activeEnd, out endTime);
                }

                var windowStart = ConvertLocalToUtc(localDate.Add(startTime));
                var windowEnd = ConvertLocalToUtc(localDate.Add(endTime));
                if (!windowStart.HasValue || !windowEnd.HasValue || windowEnd <= nowUtc)
                {
                    continue;
                }

                var lowerBound = windowStart.Value > nowUtc ? windowStart.Value : nowUtc.AddMilliseconds(1);
                var deltaSeconds = (lowerBound - anchorUtc).TotalSeconds;
                var steps = deltaSeconds <= 0 ? 0L : (long)Math.Ceiling(deltaSeconds / interval);
                var candidate = anchorUtc.AddSeconds(steps * (long)interval);
                if (candidate <= nowUtc)
                {
                    candidate = candidate.AddSeconds(interval);
                }
                if (candidate < windowStart.Value)
                {
                    var missing = (windowStart.Value - candidate).TotalSeconds;
                    candidate = candidate.AddSeconds(Math.Ceiling(missing / interval) * interval);
                }

                if (candidate < windowEnd.Value)
                {
                    return new TaskOccurrence { Task = task, DueUtc = candidate };
                }
            }
            return null;
        }

        private static TaskOccurrence GetNextFixed(
            ScreenshotTaskSettings task,
            TaskRuntimeState state,
            DateTimeOffset nowUtc)
        {
            var nowLocal = TimeZoneInfo.ConvertTime(nowUtc, TimeZoneInfo.Local);
            var times = task.schedule.times
                .Where(value => task.schedule.type != "fixedOnce"
                    || state.completedFixedTimes == null
                    || !state.completedFixedTimes.Contains(value, StringComparer.Ordinal))
                .Select(value =>
                {
                    SettingsValidator.TryParseClock(value, out var parsed);
                    return new { Text = value, Value = parsed };
                })
                .OrderBy(item => item.Value)
                .ToList();

            for (var offset = 0; offset <= 8; offset++)
            {
                var date = nowLocal.Date.AddDays(offset);
                if (!task.schedule.weekdays.Contains(DayToken(date.DayOfWeek)))
                {
                    continue;
                }
                foreach (var time in times)
                {
                    var localCandidate = date.Add(time.Value);
                    var key = localCandidate.ToString("yyyy-MM-dd'T'HH:mm:ss", CultureInfo.InvariantCulture);
                    var candidate = ConvertLocalToUtc(localCandidate);
                    if (!candidate.HasValue || candidate <= nowUtc
                        || string.Equals(key, state.lastFixedOccurrence, StringComparison.Ordinal))
                    {
                        continue;
                    }
                    return new TaskOccurrence
                    {
                        Task = task,
                        DueUtc = candidate.Value,
                        FixedOccurrenceKey = key,
                        FixedTimeSlot = task.schedule.type == "fixedOnce" ? time.Text : null
                    };
                }
            }
            return null;
        }

        private static DateTimeOffset? ConvertLocalToUtc(DateTime local)
        {
            local = DateTime.SpecifyKind(local, DateTimeKind.Unspecified);
            if (TimeZoneInfo.Local.IsInvalidTime(local))
            {
                return null;
            }
            var offset = TimeZoneInfo.Local.GetUtcOffset(local);
            if (TimeZoneInfo.Local.IsAmbiguousTime(local))
            {
                offset = TimeZoneInfo.Local.GetAmbiguousTimeOffsets(local).OrderBy(value => value).First();
            }
            return new DateTimeOffset(local, offset).ToUniversalTime();
        }
    }
}
