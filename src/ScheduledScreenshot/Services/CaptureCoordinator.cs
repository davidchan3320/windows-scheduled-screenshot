using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Drawing;
using System.Drawing.Imaging;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using System.Threading;
using System.Windows.Forms;
using ScheduledScreenshot.Models;

namespace ScheduledScreenshot.Services
{
    internal sealed class CaptureCoordinator
    {
        private readonly string _applicationDirectory;
        private readonly DiagnosticLogger _logger;
        private int _busy;

        public CaptureCoordinator(string applicationDirectory, DiagnosticLogger logger)
        {
            _applicationDirectory = applicationDirectory;
            _logger = logger;
        }

        public bool IsBusy => Volatile.Read(ref _busy) != 0;

        public CaptureBatchResult TryCapture(IList<ScreenshotTaskSettings> tasks, string reason)
        {
            if (tasks == null || tasks.Count == 0)
            {
                return new CaptureBatchResult();
            }
            if (Interlocked.CompareExchange(ref _busy, 1, 0) != 0)
            {
                foreach (var task in tasks)
                {
                    _logger.Info("TASK_SKIPPED_BUSY", "Capture was skipped because another batch is active.",
                        LogContext.ForTask(task));
                }
                return new CaptureBatchResult { Skipped = true };
            }

            var batchId = Guid.NewGuid().ToString("N");
            var stopwatch = Stopwatch.StartNew();
            var result = new CaptureBatchResult { BatchId = batchId };
            try
            {
                _logger.Info("BATCH_START", "Screenshot batch started.",
                    new LogContext
                    {
                        BatchId = batchId,
                        Values = new Dictionary<string, object>
                        {
                            ["reason"] = reason,
                            ["taskCount"] = tasks.Count
                        }
                    });

                var timestamp = DateTime.Now;
                var groups = tasks.GroupBy(BuildProfileKey).ToList();
                foreach (var group in groups)
                {
                    var representative = group.First();
                    try
                    {
                        var files = CaptureProfile(representative, timestamp, batchId);
                        result.Files.AddRange(files);
                        result.SuccessfulTaskIds.AddRange(group.Select(item => item.id));
                    }
                    catch (Exception exception)
                    {
                        result.FailedTaskIds.AddRange(group.Select(item => item.id));
                        _logger.Error("CAPTURE_PROFILE_FAILED", "A capture profile failed; other profiles will continue.",
                            exception, new LogContext
                            {
                                TaskId = representative.id,
                                TaskName = representative.name,
                                BatchId = batchId
                        });
                    }
                }
                return result;
            }
            catch (Exception exception)
            {
                foreach (var task in tasks.Where(task => !result.FailedTaskIds.Contains(task.id)))
                {
                    result.FailedTaskIds.Add(task.id);
                }
                _logger.Error("BATCH_FAILED", "The screenshot batch failed before all profiles could be processed.",
                    exception, new LogContext { BatchId = batchId });
                return result;
            }
            finally
            {
                stopwatch.Stop();
                _logger.Info("BATCH_COMPLETE", "Screenshot batch completed.",
                    new LogContext
                    {
                        BatchId = batchId,
                        DurationMs = stopwatch.ElapsedMilliseconds,
                        Values = new Dictionary<string, object>
                        {
                            ["filesWritten"] = result.Files.Count,
                            ["successfulTasks"] = result.SuccessfulTaskIds.Count,
                            ["failedTasks"] = result.FailedTaskIds.Count
                        }
                    }, flush: true);
                Interlocked.Exchange(ref _busy, 0);
            }
        }

        private IList<string> CaptureProfile(ScreenshotTaskSettings task, DateTime timestamp, string batchId)
        {
            var files = new List<string>();
            var screens = Screen.AllScreens.OrderBy(screen => screen.Bounds.Left).ThenBy(screen => screen.Bounds.Top).ToArray();
            for (var index = 0; index < screens.Length; index++)
            {
                var screen = screens[index];
                var watch = Stopwatch.StartNew();
                try
                {
                    using (var bitmap = new Bitmap(screen.Bounds.Width, screen.Bounds.Height, PixelFormat.Format32bppArgb))
                    using (var graphics = Graphics.FromImage(bitmap))
                    {
                        graphics.CopyFromScreen(screen.Bounds.Left, screen.Bounds.Top, 0, 0, screen.Bounds.Size,
                            CopyPixelOperation.SourceCopy | CopyPixelOperation.CaptureBlt);
                        if (task.capture.includeCursor)
                        {
                            CursorRenderer.Draw(graphics, screen.Bounds);
                        }
                        var path = SaveBitmap(bitmap, task, timestamp, screen.DeviceName, index + 1);
                        files.Add(path);
                        watch.Stop();
                        _logger.Debug("MONITOR_CAPTURE_SUCCESS", "Monitor screenshot was written.",
                            new LogContext
                            {
                                TaskId = task.id,
                                TaskName = task.name,
                                BatchId = batchId,
                                Display = screen.DeviceName,
                                FilePath = path,
                                DurationMs = watch.ElapsedMilliseconds
                            });
                    }
                }
                catch (Exception exception)
                {
                    watch.Stop();
                    _logger.Error("MONITOR_CAPTURE_FAILED", "A monitor could not be captured.", exception,
                        new LogContext
                        {
                            TaskId = task.id,
                            TaskName = task.name,
                            BatchId = batchId,
                            Display = screen.DeviceName,
                            DurationMs = watch.ElapsedMilliseconds
                        });
                }
            }
            if (files.Count == 0)
            {
                throw new InvalidOperationException("No monitor screenshots were written.");
            }
            return files;
        }

        private string SaveBitmap(
            Bitmap bitmap,
            ScreenshotTaskSettings task,
            DateTime timestamp,
            string displayName,
            int displayIndex)
        {
            var output = SettingsValidator.ResolveDirectory(task.capture.outputFolder, _applicationDirectory);
            var directory = Path.Combine(output, timestamp.ToString("yyyy-MM-dd", CultureInfo.InvariantCulture));
            Directory.CreateDirectory(directory);
            var stem = FileNameTemplate.Expand(task.capture.fileNameTemplate, task, timestamp, displayName, displayIndex);
            var extension = task.capture.imageFormat == "png" ? ".png" : ".jpg";

            for (var collision = 0; collision < 100000; collision++)
            {
                var suffix = collision == 0 ? string.Empty : "_" + collision.ToString("000", CultureInfo.InvariantCulture);
                var destination = Path.Combine(directory, stem + suffix + extension);
                if (destination.Length > 240)
                {
                    throw new PathTooLongException("Expanded screenshot path exceeds 240 characters: " + destination);
                }
                if (File.Exists(destination))
                {
                    continue;
                }

                var temporary = Path.Combine(directory, ".capture-" + Guid.NewGuid().ToString("N") + ".tmp");
                try
                {
                    SaveEncoded(bitmap, temporary, task.capture);
                    try
                    {
                        File.Move(temporary, destination);
                        if (collision > 0)
                        {
                            _logger.Debug("FILENAME_COLLISION", "A filename collision suffix was applied.",
                                new LogContext { TaskId = task.id, TaskName = task.name, FilePath = destination });
                        }
                        return destination;
                    }
                    catch (IOException)
                    {
                        if (!File.Exists(destination))
                        {
                            throw;
                        }
                    }
                }
                finally
                {
                    if (File.Exists(temporary))
                    {
                        File.Delete(temporary);
                    }
                }
            }
            throw new IOException("Unable to allocate a unique screenshot filename.");
        }

        private static void SaveEncoded(Bitmap bitmap, string path, CaptureSettings settings)
        {
            if (settings.imageFormat == "png")
            {
                bitmap.Save(path, ImageFormat.Png);
                return;
            }
            var encoder = ImageCodecInfo.GetImageEncoders().First(item => item.FormatID == ImageFormat.Jpeg.Guid);
            using (var parameters = new EncoderParameters(1))
            {
                parameters.Param[0] = new EncoderParameter(Encoder.Quality, (long)settings.jpegQuality);
                bitmap.Save(path, encoder, parameters);
            }
        }

        private string BuildProfileKey(ScreenshotTaskSettings task)
        {
            var output = SettingsValidator.ResolveDirectory(task.capture.outputFolder, _applicationDirectory)
                .TrimEnd(Path.DirectorySeparatorChar).ToUpperInvariant();
            var effectiveTemplate = FileNameTemplate.ResolveTaskSpecificTemplate(task.capture.fileNameTemplate, task);
            return string.Join("|", output, effectiveTemplate, task.capture.imageFormat,
                task.capture.jpegQuality.ToString(CultureInfo.InvariantCulture), task.capture.includeCursor ? "1" : "0");
        }

        private static class CursorRenderer
        {
            private const int CursorShowing = 0x00000001;
            private const int DrawNormal = 0x0003;

            public static void Draw(Graphics graphics, Rectangle screenBounds)
            {
                var info = new CursorInfo { cbSize = Marshal.SizeOf(typeof(CursorInfo)) };
                if (!GetCursorInfo(ref info) || (info.flags & CursorShowing) == 0 || !screenBounds.Contains(info.ptScreenPos.X, info.ptScreenPos.Y))
                {
                    return;
                }

                var iconInfo = new IconInfo();
                if (!GetIconInfo(info.hCursor, ref iconInfo))
                {
                    return;
                }
                try
                {
                    var x = info.ptScreenPos.X - screenBounds.Left - (int)iconInfo.xHotspot;
                    var y = info.ptScreenPos.Y - screenBounds.Top - (int)iconInfo.yHotspot;
                    var hdc = graphics.GetHdc();
                    try
                    {
                        DrawIconEx(hdc, x, y, info.hCursor, 0, 0, 0, IntPtr.Zero, DrawNormal);
                    }
                    finally
                    {
                        graphics.ReleaseHdc(hdc);
                    }
                }
                finally
                {
                    if (iconInfo.hbmColor != IntPtr.Zero) DeleteObject(iconInfo.hbmColor);
                    if (iconInfo.hbmMask != IntPtr.Zero) DeleteObject(iconInfo.hbmMask);
                }
            }

            [StructLayout(LayoutKind.Sequential)]
            private struct Point
            {
                public int X;
                public int Y;
            }

            [StructLayout(LayoutKind.Sequential)]
            private struct CursorInfo
            {
                public int cbSize;
                public int flags;
                public IntPtr hCursor;
                public Point ptScreenPos;
            }

            [StructLayout(LayoutKind.Sequential)]
            private struct IconInfo
            {
                [MarshalAs(UnmanagedType.Bool)] public bool fIcon;
                public uint xHotspot;
                public uint yHotspot;
                public IntPtr hbmMask;
                public IntPtr hbmColor;
            }

            [DllImport("user32.dll")]
            [return: MarshalAs(UnmanagedType.Bool)]
            private static extern bool GetCursorInfo(ref CursorInfo cursorInfo);

            [DllImport("user32.dll")]
            [return: MarshalAs(UnmanagedType.Bool)]
            private static extern bool GetIconInfo(IntPtr icon, ref IconInfo iconInfo);

            [DllImport("user32.dll")]
            [return: MarshalAs(UnmanagedType.Bool)]
            private static extern bool DrawIconEx(IntPtr hdc, int x, int y, IntPtr icon, int width, int height,
                int step, IntPtr brush, int flags);

            [DllImport("gdi32.dll")]
            [return: MarshalAs(UnmanagedType.Bool)]
            private static extern bool DeleteObject(IntPtr handle);
        }
    }

    internal sealed class CaptureBatchResult
    {
        public string BatchId { get; set; }
        public bool Skipped { get; set; }
        public List<string> Files { get; } = new List<string>();
        public List<string> SuccessfulTaskIds { get; } = new List<string>();
        public List<string> FailedTaskIds { get; } = new List<string>();
    }
}
