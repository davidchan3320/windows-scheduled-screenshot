using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Linq;
using ScheduledScreenshot.Models;

namespace ScheduledScreenshot.Services
{
    internal static class SettingsValidator
    {
        private static readonly HashSet<string> ValidDays = new HashSet<string>(StringComparer.Ordinal)
        {
            "Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"
        };

        public static ValidationResult Validate(AppSettings settings, string applicationDirectory, bool verifyWritablePaths)
        {
            var result = new ValidationResult();
            if (settings == null)
            {
                result.Errors.Add("Configuration cannot be null.");
                return result;
            }

            if (settings.schemaVersion != 1)
            {
                result.Errors.Add("Only schemaVersion 1 is supported.");
            }

            ValidateLogging(settings.logging, applicationDirectory, result);
            if (settings.tasks == null)
            {
                result.Errors.Add("tasks is required.");
                return result;
            }
            if (settings.tasks.Count > 100)
            {
                result.Errors.Add("No more than 100 tasks are allowed.");
            }

            var ids = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
            var names = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
            for (var index = 0; index < settings.tasks.Count; index++)
            {
                ValidateTask(settings.tasks[index], index, applicationDirectory, verifyWritablePaths, ids, names, result);
            }
            return result;
        }

        public static string ResolveDirectory(string value, string applicationDirectory)
        {
            var expanded = Environment.ExpandEnvironmentVariables(value ?? string.Empty);
            if (!Path.IsPathRooted(expanded))
            {
                expanded = Path.Combine(applicationDirectory, expanded);
            }
            return Path.GetFullPath(expanded);
        }

        public static bool TryParseClock(string value, out TimeSpan result)
        {
            return TimeSpan.TryParseExact(value, @"hh\:mm\:ss", CultureInfo.InvariantCulture, out result)
                   && result >= TimeSpan.Zero
                   && result < TimeSpan.FromDays(1);
        }

        public static bool TryParseLocalDateTime(string value, out DateTime result)
        {
            return DateTime.TryParseExact(value, "yyyy-MM-dd'T'HH:mm:ss", CultureInfo.InvariantCulture,
                DateTimeStyles.None, out result);
        }

        private static void ValidateLogging(LoggingSettings logging, string applicationDirectory, ValidationResult result)
        {
            if (logging == null)
            {
                result.Errors.Add("logging is required.");
                return;
            }
            if (logging.level != "error" && logging.level != "info" && logging.level != "debug")
            {
                result.Errors.Add("logging.level must be error, info, or debug.");
            }
            if (logging.maxFileSizeMb < 1 || logging.maxFileSizeMb > 100)
            {
                result.Errors.Add("logging.maxFileSizeMb must be between 1 and 100.");
            }
            if (logging.retainedFiles < 1 || logging.retainedFiles > 20)
            {
                result.Errors.Add("logging.retainedFiles must be between 1 and 20.");
            }
            if (string.IsNullOrWhiteSpace(logging.directory) || Path.IsPathRooted(logging.directory))
            {
                result.Errors.Add("logging.directory must be a relative child directory.");
                return;
            }
            var logSegments = logging.directory.Split(new[] { '\\', '/' }, StringSplitOptions.RemoveEmptyEntries);
            if (logSegments.Any(segment => segment == ".."))
            {
                result.Errors.Add("logging.directory cannot contain parent-directory traversal.");
                return;
            }

            try
            {
                var basePath = Path.GetFullPath(applicationDirectory).TrimEnd(Path.DirectorySeparatorChar) + Path.DirectorySeparatorChar;
                var logPath = Path.GetFullPath(Path.Combine(applicationDirectory, logging.directory)).TrimEnd(Path.DirectorySeparatorChar) + Path.DirectorySeparatorChar;
                if (!logPath.StartsWith(basePath, StringComparison.OrdinalIgnoreCase))
                {
                    result.Errors.Add("logging.directory cannot escape the application directory.");
                }
            }
            catch (Exception exception)
            {
                result.Errors.Add("logging.directory is invalid: " + exception.Message);
            }
        }

        private static void ValidateTask(
            ScreenshotTaskSettings task,
            int index,
            string applicationDirectory,
            bool verifyWritablePaths,
            HashSet<string> ids,
            HashSet<string> names,
            ValidationResult result)
        {
            var prefix = "tasks[" + index + "]";
            if (task == null)
            {
                result.Errors.Add(prefix + " cannot be null.");
                return;
            }

            if (!Guid.TryParse(task.id, out _))
            {
                result.Errors.Add(prefix + ".id must be a GUID.");
            }
            else if (!ids.Add(task.id))
            {
                result.Errors.Add(prefix + ".id must be unique.");
            }

            if (string.IsNullOrWhiteSpace(task.name) || task.name.Trim().Length > 64)
            {
                result.Errors.Add(prefix + ".name must contain 1-64 characters.");
            }
            else if (!names.Add(task.name.Trim()))
            {
                result.Errors.Add(prefix + ".name must be unique.");
            }

            ValidateCapture(task.capture, prefix, applicationDirectory, verifyWritablePaths && task.enabled, result);
            ValidateSchedule(task.schedule, prefix, result);
            ValidateStop(task.stopCondition, prefix, task.enabled, result);
        }

        private static void ValidateCapture(
            CaptureSettings capture,
            string prefix,
            string applicationDirectory,
            bool verifyWritablePaths,
            ValidationResult result)
        {
            if (capture == null)
            {
                result.Errors.Add(prefix + ".capture is required.");
                return;
            }
            if (capture.imageFormat != "jpeg" && capture.imageFormat != "png")
            {
                result.Errors.Add(prefix + ".capture.imageFormat must be jpeg or png.");
            }
            if (capture.jpegQuality < 50 || capture.jpegQuality > 100)
            {
                result.Errors.Add(prefix + ".capture.jpegQuality must be between 50 and 100.");
            }
            foreach (var error in FileNameTemplate.Validate(capture.fileNameTemplate))
            {
                result.Errors.Add(prefix + ".capture.fileNameTemplate: " + error);
            }
            if (string.IsNullOrWhiteSpace(capture.outputFolder))
            {
                result.Errors.Add(prefix + ".capture.outputFolder is required.");
                return;
            }

            try
            {
                var output = ResolveDirectory(capture.outputFolder, applicationDirectory);
                var sampleName = FileNameTemplate.Expand(capture.fileNameTemplate,
                    new ScreenshotTaskSettings { id = Guid.NewGuid().ToString(), name = "Sample task" },
                    DateTime.Now, @"\\.\DISPLAY1", 1) + "." + (capture.imageFormat == "png" ? "png" : "jpg");
                if (Path.Combine(output, DateTime.Now.ToString("yyyy-MM-dd"), sampleName).Length > 240)
                {
                    result.Errors.Add(prefix + ".capture produces a path longer than 240 characters.");
                }
                if (verifyWritablePaths)
                {
                    VerifyWritable(output);
                }
            }
            catch (Exception exception)
            {
                result.Errors.Add(prefix + ".capture.outputFolder is not writable: " + exception.Message);
            }
        }

        private static void ValidateSchedule(ScheduleSettings schedule, string prefix, ValidationResult result)
        {
            if (schedule == null)
            {
                result.Errors.Add(prefix + ".schedule is required.");
                return;
            }
            if (schedule.type != "interval" && schedule.type != "fixed")
            {
                result.Errors.Add(prefix + ".schedule.type must be interval or fixed.");
            }
            if (schedule.weekdays == null || schedule.weekdays.Count == 0 || schedule.weekdays.Any(day => !ValidDays.Contains(day))
                || schedule.weekdays.Distinct(StringComparer.Ordinal).Count() != schedule.weekdays.Count)
            {
                result.Errors.Add(prefix + ".schedule.weekdays must contain unique valid weekday tokens.");
            }

            if (schedule.type == "interval")
            {
                if (schedule.intervalSeconds < 1 || schedule.intervalSeconds > 86400)
                {
                    result.Errors.Add(prefix + ".schedule.intervalSeconds must be between 1 and 86400.");
                }
                if (schedule.activeHoursEnabled)
                {
                    if (!TryParseClock(schedule.activeStart, out var start) || !TryParseClock(schedule.activeEnd, out var end) || start >= end)
                    {
                        result.Errors.Add(prefix + ".schedule active hours must be valid same-day HH:mm:ss values with start before end.");
                    }
                }
            }
            else if (schedule.type == "fixed")
            {
                if (schedule.times == null || schedule.times.Count == 0)
                {
                    result.Errors.Add(prefix + ".schedule.times must contain at least one time.");
                }
                else
                {
                    if (schedule.times.Any(value => !TryParseClock(value, out _)))
                    {
                        result.Errors.Add(prefix + ".schedule.times must use HH:mm:ss.");
                    }
                    if (schedule.times.Distinct(StringComparer.Ordinal).Count() != schedule.times.Count)
                    {
                        result.Errors.Add(prefix + ".schedule.times must be unique.");
                    }
                }
            }
        }

        private static void ValidateStop(StopConditionSettings stop, string prefix, bool enabled, ValidationResult result)
        {
            if (stop == null)
            {
                result.Errors.Add(prefix + ".stopCondition is required.");
                return;
            }
            if (stop.mode != "none" && stop.mode != "at" && stop.mode != "duration")
            {
                result.Errors.Add(prefix + ".stopCondition.mode must be none, at, or duration.");
            }
            if (stop.mode == "at")
            {
                if (!TryParseLocalDateTime(stop.endAtLocal, out var end)
                    || TimeZoneInfo.Local.IsInvalidTime(DateTime.SpecifyKind(end, DateTimeKind.Unspecified))
                    || (enabled && end <= DateTime.Now))
                {
                    result.Errors.Add(prefix + ".stopCondition.endAtLocal must be a future yyyy-MM-ddTHH:mm:ss value for an enabled task.");
                }
            }
            if (stop.mode == "duration" && (!stop.durationSeconds.HasValue || stop.durationSeconds < 1 || stop.durationSeconds > 31536000))
            {
                result.Errors.Add(prefix + ".stopCondition.durationSeconds must be between 1 and 31536000.");
            }
        }

        private static void VerifyWritable(string directory)
        {
            Directory.CreateDirectory(directory);
            var probe = Path.Combine(directory, ".scheduled-screenshot-write-test-" + Guid.NewGuid().ToString("N") + ".tmp");
            using (File.Create(probe, 1, FileOptions.DeleteOnClose))
            {
            }
        }
    }
}
