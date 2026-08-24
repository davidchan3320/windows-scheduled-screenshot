using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Text;
using ScheduledScreenshot.Models;

namespace ScheduledScreenshot.Services
{
    internal sealed class DiagnosticLogger : IDisposable
    {
        private readonly object _sync = new object();
        private readonly string _applicationDirectory;
        private LoggingSettings _settings = new LoggingSettings();
        private StreamWriter _writer;
        private string _currentDate;
        private string _currentPath;
        private DateTime _retryAfterUtc;
        private bool _notifiedFailure;

        public DiagnosticLogger(string applicationDirectory)
        {
            _applicationDirectory = applicationDirectory;
        }

        public event Action<string> WriteFailed;

        public string LogDirectory
        {
            get
            {
                lock (_sync)
                {
                    return Path.GetFullPath(Path.Combine(_applicationDirectory, _settings.directory ?? "logs"));
                }
            }
        }

        public void Configure(LoggingSettings settings)
        {
            lock (_sync)
            {
                CloseWriter();
                _settings = settings ?? new LoggingSettings();
                _retryAfterUtc = DateTime.MinValue;
                _notifiedFailure = false;
            }
        }

        public void Error(string eventId, string message, Exception exception = null, LogContext context = null)
        {
            Write("error", eventId, message, exception, context, true);
        }

        public void Info(string eventId, string message, LogContext context = null, bool flush = false)
        {
            Write("info", eventId, message, null, context, flush);
        }

        public void Debug(string eventId, string message, LogContext context = null)
        {
            Write("debug", eventId, message, null, context, false);
        }

        public void Dispose()
        {
            lock (_sync)
            {
                CloseWriter();
            }
        }

        private void Write(string level, string eventId, string message, Exception exception, LogContext context, bool flush)
        {
            Action<string> failureCallback = null;
            string failureMessage = null;
            lock (_sync)
            {
                if (!ShouldWrite(level) || DateTime.UtcNow < _retryAfterUtc)
                {
                    return;
                }

                try
                {
                    var entry = new LogEntry
                    {
                        timestampUtc = DateTime.UtcNow.ToString("o", CultureInfo.InvariantCulture),
                        level = level,
                        eventId = eventId,
                        message = message,
                        taskId = context?.TaskId,
                        taskName = context?.TaskName,
                        batchId = context?.BatchId,
                        display = context?.Display,
                        filePath = context?.FilePath,
                        durationMs = context?.DurationMs,
                        exceptionType = exception?.GetType().FullName,
                        hResult = exception?.HResult,
                        stackTrace = exception?.ToString(),
                        context = context?.Values
                    };
                    var line = JsonUtility.Serialize(entry);
                    EnsureWriter(Encoding.UTF8.GetByteCount(line) + Environment.NewLine.Length);
                    _writer.WriteLine(line);
                    if (flush || level == "error")
                    {
                        _writer.Flush();
                    }
                    _notifiedFailure = false;
                }
                catch (Exception writeException)
                {
                    CloseWriter();
                    _retryAfterUtc = DateTime.UtcNow.AddMinutes(1);
                    if (!_notifiedFailure)
                    {
                        _notifiedFailure = true;
                        failureMessage = "Diagnostic logging failed: " + writeException.Message;
                        failureCallback = WriteFailed;
                    }
                }
            }

            failureCallback?.Invoke(failureMessage);
        }

        private bool ShouldWrite(string level)
        {
            if (level == "error")
            {
                return true;
            }
            if (_settings.level == "error")
            {
                return false;
            }
            return level != "debug" || _settings.level == "debug";
        }

        private void EnsureWriter(int incomingBytes)
        {
            var localDate = DateTime.Now.ToString("yyyyMMdd", CultureInfo.InvariantCulture);
            var maxBytes = (long)Math.Max(1, _settings.maxFileSizeMb) * 1024L * 1024L;
            if (_writer != null && (_currentDate != localDate || new FileInfo(_currentPath).Length + incomingBytes > maxBytes))
            {
                CloseWriter();
            }
            if (_writer != null)
            {
                return;
            }

            var directory = LogDirectory;
            Directory.CreateDirectory(directory);
            var sequence = 1;
            while (true)
            {
                var candidate = Path.Combine(directory,
                    "screen-capture-" + localDate + "-" + sequence.ToString("000", CultureInfo.InvariantCulture) + ".jsonl");
                if (!File.Exists(candidate) || new FileInfo(candidate).Length + incomingBytes <= maxBytes)
                {
                    _currentPath = candidate;
                    break;
                }
                sequence++;
            }

            _currentDate = localDate;
            _writer = new StreamWriter(new FileStream(_currentPath, FileMode.Append, FileAccess.Write, FileShare.Read),
                new UTF8Encoding(false));
            RemoveOldLogs(directory);
        }

        private void RemoveOldLogs(string directory)
        {
            var files = Directory.GetFiles(directory, "screen-capture-*.jsonl")
                .OrderByDescending(path => Path.GetFileName(path), StringComparer.OrdinalIgnoreCase)
                .ToList();
            foreach (var file in files.Skip(Math.Max(1, _settings.retainedFiles)))
            {
                if (!string.Equals(file, _currentPath, StringComparison.OrdinalIgnoreCase))
                {
                    File.Delete(file);
                }
            }
        }

        private void CloseWriter()
        {
            if (_writer == null)
            {
                return;
            }
            try
            {
                _writer.Flush();
                _writer.Dispose();
            }
            catch
            {
                // Logging must never prevent shutdown or capture work.
            }
            finally
            {
                _writer = null;
                _currentPath = null;
                _currentDate = null;
            }
        }

        private sealed class LogEntry
        {
            public string timestampUtc { get; set; }
            public string level { get; set; }
            public string eventId { get; set; }
            public string message { get; set; }
            public string taskId { get; set; }
            public string taskName { get; set; }
            public string batchId { get; set; }
            public string display { get; set; }
            public string filePath { get; set; }
            public long? durationMs { get; set; }
            public string exceptionType { get; set; }
            public int? hResult { get; set; }
            public string stackTrace { get; set; }
            public IDictionary<string, object> context { get; set; }
        }
    }

    internal sealed class LogContext
    {
        public string TaskId { get; set; }
        public string TaskName { get; set; }
        public string BatchId { get; set; }
        public string Display { get; set; }
        public string FilePath { get; set; }
        public long? DurationMs { get; set; }
        public IDictionary<string, object> Values { get; set; }

        public static LogContext ForTask(ScreenshotTaskSettings task)
        {
            return new LogContext { TaskId = task?.id, TaskName = task?.name };
        }
    }
}
