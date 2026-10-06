using System;
using System.Collections.Generic;

namespace ScheduledScreenshot.Models
{
    public sealed class AppSettings
    {
        public int schemaVersion { get; set; } = 1;
        public bool paused { get; set; }
        public LoggingSettings logging { get; set; } = new LoggingSettings();
        public List<ScreenshotTaskSettings> tasks { get; set; } = new List<ScreenshotTaskSettings>();
    }

    public sealed class LoggingSettings
    {
        public string level { get; set; } = "info";
        public string directory { get; set; } = "logs";
        public int maxFileSizeMb { get; set; } = 5;
        public int retainedFiles { get; set; } = 5;
    }

    public sealed class ScreenshotTaskSettings
    {
        public string id { get; set; } = Guid.NewGuid().ToString();
        public string name { get; set; } = "New task";
        public bool enabled { get; set; }
        public CaptureSettings capture { get; set; } = new CaptureSettings();
        public ScheduleSettings schedule { get; set; } = new ScheduleSettings();
        public StopConditionSettings stopCondition { get; set; } = new StopConditionSettings();
    }

    public sealed class CaptureSettings
    {
        public string outputFolder { get; set; } = @"%USERPROFILE%\Pictures\Scheduled Screenshots";
        public string fileNameTemplate { get; set; } = "{timestamp}_{display}";
        public string imageFormat { get; set; } = "jpeg";
        public int jpegQuality { get; set; } = 85;
        public bool includeCursor { get; set; }
    }

    public sealed class ScheduleSettings
    {
        public string type { get; set; } = "interval";
        public int intervalSeconds { get; set; } = 60;
        public List<string> weekdays { get; set; } = new List<string>
        {
            "Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"
        };
        public bool activeHoursEnabled { get; set; }
        public string activeStart { get; set; } = "09:00:00";
        public string activeEnd { get; set; } = "17:00:00";
        public List<string> times { get; set; } = new List<string> { "09:00:00" };
    }

    public sealed class StopConditionSettings
    {
        public string mode { get; set; } = "none";
        public string endAtLocal { get; set; }
        public int? durationSeconds { get; set; }
        public int? captureCount { get; set; }
    }

    public sealed class ValidationResult
    {
        public List<string> Errors { get; } = new List<string>();
        public List<string> Warnings { get; } = new List<string>();
        public bool IsValid => Errors.Count == 0;
    }

    public sealed class RuntimeState
    {
        public Dictionary<string, TaskRuntimeState> tasks { get; set; } =
            new Dictionary<string, TaskRuntimeState>(StringComparer.OrdinalIgnoreCase);
    }

    public sealed class TaskRuntimeState
    {
        public string intervalAnchorUtc { get; set; }
        public string scheduleSignature { get; set; }
        public string durationDeadlineUtc { get; set; }
        public string durationSignature { get; set; }
        public string lastFixedOccurrence { get; set; }
        public List<string> completedFixedTimes { get; set; } = new List<string>();
        public int completedCaptures { get; set; }
        public string countSignature { get; set; }
    }

    public sealed class TaskOccurrence
    {
        public ScreenshotTaskSettings Task { get; set; }
        public DateTimeOffset DueUtc { get; set; }
        public string FixedOccurrenceKey { get; set; }
        public string FixedTimeSlot { get; set; }
        public string ScheduleSignature { get; set; }
        public string RuntimeAnchorUtc { get; set; }
        public string CountSignature { get; set; }
    }
}
