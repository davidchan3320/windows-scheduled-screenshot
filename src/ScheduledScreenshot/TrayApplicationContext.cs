using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Drawing;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Threading;
using System.Windows.Forms;
using ScheduledScreenshot.Models;
using ScheduledScreenshot.Services;

namespace ScheduledScreenshot
{
    internal sealed class TrayApplicationContext : ApplicationContext
    {
        private readonly string _applicationDirectory;
        private readonly Control _dispatcher;
        private readonly DiagnosticLogger _logger;
        private readonly ConfigurationService _configuration;
        private readonly RuntimeStateStore _runtimeState;
        private readonly CaptureCoordinator _capture;
        private readonly SchedulerService _scheduler;
        private readonly SessionMonitor _sessionMonitor;
        private readonly NotifyIcon _notifyIcon;
        private readonly ToolStripMenuItem _statusItem;
        private readonly ToolStripMenuItem _captureNowItem;
        private readonly ToolStripMenuItem _pauseItem;
        private readonly ToolStripMenuItem _openOutputItem;
        private bool _exiting;

        public TrayApplicationContext()
        {
            _applicationDirectory = AppDomain.CurrentDomain.BaseDirectory.TrimEnd(Path.DirectorySeparatorChar);
            _dispatcher = new Control();
            _dispatcher.CreateControl();

            _logger = new DiagnosticLogger(_applicationDirectory);
            _configuration = new ConfigurationService(_applicationDirectory, _logger, SynchronizationContext.Current);
            _runtimeState = new RuntimeStateStore(_applicationDirectory, _logger);
            _capture = new CaptureCoordinator(_applicationDirectory, _logger);
            _scheduler = new SchedulerService(_configuration, _runtimeState, _capture, _logger);
            _sessionMonitor = new SessionMonitor();

            _statusItem = new ToolStripMenuItem("Starting…") { Enabled = false };
            _captureNowItem = new ToolStripMenuItem("Capture Now");
            _pauseItem = new ToolStripMenuItem("Pause All");
            _openOutputItem = new ToolStripMenuItem("Open Output Folder");

            var menu = BuildMenu();
            _notifyIcon = new NotifyIcon
            {
                Icon = SystemIcons.Application,
                Text = "Scheduled Screenshot",
                Visible = true,
                ContextMenuStrip = menu
            };

            _logger.WriteFailed += OnLogWriteFailed;
            _configuration.SettingsChanged += OnSettingsChanged;
            _configuration.SettingsRejected += message => ShowWarning("Configuration rejected", message);
            _scheduler.SchedulerStatusChanged += UpdateStatusSafe;
            _sessionMonitor.AvailabilityChanged += (available, reason) => _scheduler.SetCaptureAllowed(available, reason);
            _sessionMonitor.ClockChanged += _scheduler.NotifyClockChanged;
            Application.ThreadException += OnThreadException;
            AppDomain.CurrentDomain.UnhandledException += OnUnhandledException;

            _logger.Info("APP_START", "Scheduled Screenshot started.",
                new LogContext
                {
                    Values = new Dictionary<string, object>
                    {
                        ["version"] = typeof(TrayApplicationContext).Assembly.GetName().Version.ToString(),
                        ["applicationDirectory"] = _applicationDirectory
                    }
                }, true);
            UpdateStatus();
        }

        protected override void ExitThreadCore()
        {
            if (_exiting) return;
            _exiting = true;
            _logger.Info("APP_EXIT", "Scheduled Screenshot is exiting.", flush: true);
            Application.ThreadException -= OnThreadException;
            AppDomain.CurrentDomain.UnhandledException -= OnUnhandledException;
            _sessionMonitor.Dispose();
            _scheduler.Dispose();
            _configuration.Dispose();
            _logger.Dispose();
            _notifyIcon.Visible = false;
            _notifyIcon.Dispose();
            _dispatcher.Dispose();
            base.ExitThreadCore();
        }

        private ContextMenuStrip BuildMenu()
        {
            var menu = new ContextMenuStrip();
            menu.Opening += (sender, args) => RefreshDynamicMenus();
            menu.Items.Add(_statusItem);
            menu.Items.Add(new ToolStripSeparator());
            menu.Items.Add(_captureNowItem);
            menu.Items.Add(_pauseItem);
            menu.Items.Add(new ToolStripSeparator());

            var configureItem = new ToolStripMenuItem("Configure in Browser");
            configureItem.Click += (sender, args) => OpenWithShell(Path.Combine(_applicationDirectory, "config-editor.html"));
            menu.Items.Add(configureItem);

            var editItem = new ToolStripMenuItem("Edit JSON in Notepad");
            editItem.Click += (sender, args) => StartProcess("notepad.exe", Quote(_configuration.SettingsPath));
            menu.Items.Add(editItem);
            menu.Items.Add(_openOutputItem);

            var logsItem = new ToolStripMenuItem("Open Logs");
            logsItem.Click += (sender, args) =>
            {
                Directory.CreateDirectory(_logger.LogDirectory);
                OpenWithShell(_logger.LogDirectory);
            };
            menu.Items.Add(logsItem);
            menu.Items.Add(new ToolStripSeparator());

            var exitItem = new ToolStripMenuItem("Exit");
            exitItem.Click += (sender, args) => ExitThread();
            menu.Items.Add(exitItem);
            return menu;
        }

        private void RefreshDynamicMenus()
        {
            var settings = _configuration.Current;
            _captureNowItem.DropDownItems.Clear();
            _openOutputItem.DropDownItems.Clear();
            foreach (var task in settings.tasks.Where(item => item.enabled))
            {
                var captureTask = task;
                var captureItem = new ToolStripMenuItem(task.name);
                captureItem.Click += (sender, args) => CaptureNow(captureTask);
                _captureNowItem.DropDownItems.Add(captureItem);
            }
            foreach (var task in settings.tasks)
            {
                var outputTask = task;
                var outputItem = new ToolStripMenuItem(task.name);
                outputItem.Click += (sender, args) =>
                    OpenWithShell(SettingsValidator.ResolveDirectory(outputTask.capture.outputFolder, _applicationDirectory));
                _openOutputItem.DropDownItems.Add(outputItem);
            }
            _captureNowItem.Enabled = _captureNowItem.DropDownItems.Count > 0;
            _openOutputItem.Enabled = _openOutputItem.DropDownItems.Count > 0;
            _pauseItem.Text = settings.paused ? "Resume All" : "Pause All";
            _pauseItem.Click -= TogglePause;
            _pauseItem.Click += TogglePause;
            UpdateStatus();
        }

        private void CaptureNow(ScreenshotTaskSettings task)
        {
            ThreadPool.QueueUserWorkItem(_ =>
            {
                var result = _capture.TryCapture(new List<ScreenshotTaskSettings> { task }, "manual");
                if (!result.Skipped && result.Files.Count > 0)
                {
                    ShowInfoSafe("Capture complete", result.Files.Count.ToString(CultureInfo.InvariantCulture) + " screenshot(s) written.");
                }
                else if (result.Skipped)
                {
                    ShowWarningSafe("Capture skipped", "Another capture batch is already active.");
                }
            });
        }

        private void TogglePause(object sender, EventArgs arguments)
        {
            var settings = _configuration.Current;
            _configuration.SetPaused(!settings.paused);
        }

        private void OnSettingsChanged(AppSettings settings)
        {
            UpdateStatusSafe();
        }

        private void UpdateStatusSafe()
        {
            if (_dispatcher.IsDisposed) return;
            if (_dispatcher.InvokeRequired)
            {
                _dispatcher.BeginInvoke((Action)UpdateStatus);
            }
            else
            {
                UpdateStatus();
            }
        }

        private void UpdateStatus()
        {
            if (_exiting) return;
            var status = _scheduler.GetStatus();
            string text;
            if (status.Paused)
            {
                text = "Paused";
            }
            else if (!status.CaptureAllowed)
            {
                text = "Waiting for an unlocked session";
            }
            else if (status.NextOccurrenceUtc.HasValue)
            {
                var local = TimeZoneInfo.ConvertTime(status.NextOccurrenceUtc.Value, TimeZoneInfo.Local);
                text = "Next: " + status.NextTaskName + " at " + local.ToString("yyyy-MM-dd HH:mm:ss");
            }
            else
            {
                text = "No scheduled captures";
            }
            _statusItem.Text = text;
            _notifyIcon.Text = text.Length <= 63 ? text : text.Substring(0, 60) + "…";
        }

        private void OnLogWriteFailed(string message)
        {
            ShowWarningSafe("Logging unavailable", message);
        }

        private void OnThreadException(object sender, ThreadExceptionEventArgs arguments)
        {
            _logger.Error("UNHANDLED_UI_ERROR", "An unhandled UI error occurred.", arguments.Exception);
            ShowWarning("Application error", arguments.Exception.Message);
        }

        private void OnUnhandledException(object sender, UnhandledExceptionEventArgs arguments)
        {
            _logger.Error("UNHANDLED_ERROR", "An unhandled application error occurred.", arguments.ExceptionObject as Exception);
        }

        private void ShowInfoSafe(string title, string message)
        {
            ShowBalloonSafe(title, message, ToolTipIcon.Info);
        }

        private void ShowWarningSafe(string title, string message)
        {
            ShowBalloonSafe(title, message, ToolTipIcon.Warning);
        }

        private void ShowWarning(string title, string message)
        {
            ShowBalloon(title, message, ToolTipIcon.Warning);
        }

        private void ShowBalloonSafe(string title, string message, ToolTipIcon icon)
        {
            if (_dispatcher.IsDisposed) return;
            if (_dispatcher.InvokeRequired)
            {
                _dispatcher.BeginInvoke((Action)(() => ShowBalloon(title, message, icon)));
            }
            else
            {
                ShowBalloon(title, message, icon);
            }
        }

        private void ShowBalloon(string title, string message, ToolTipIcon icon)
        {
            if (_exiting || _notifyIcon == null) return;
            _notifyIcon.BalloonTipTitle = title;
            _notifyIcon.BalloonTipText = message.Length > 240 ? message.Substring(0, 237) + "…" : message;
            _notifyIcon.BalloonTipIcon = icon;
            _notifyIcon.ShowBalloonTip(5000);
        }

        private void OpenWithShell(string path)
        {
            try
            {
                if (!File.Exists(path) && !Directory.Exists(path))
                {
                    throw new FileNotFoundException("The requested file or directory does not exist.", path);
                }
                Process.Start(new ProcessStartInfo(path) { UseShellExecute = true });
            }
            catch (Exception exception)
            {
                _logger.Error("SHELL_OPEN_FAILED", "A file or folder could not be opened.", exception,
                    new LogContext { FilePath = path });
                ShowWarning("Unable to open", exception.Message);
            }
        }

        private void StartProcess(string fileName, string arguments)
        {
            try
            {
                Process.Start(new ProcessStartInfo(fileName, arguments) { UseShellExecute = true });
            }
            catch (Exception exception)
            {
                _logger.Error("PROCESS_START_FAILED", "An external process could not be started.", exception);
                ShowWarning("Unable to start", exception.Message);
            }
        }

        private static string Quote(string value)
        {
            return "\"" + value.Replace("\"", "\\\"") + "\"";
        }
    }
}
