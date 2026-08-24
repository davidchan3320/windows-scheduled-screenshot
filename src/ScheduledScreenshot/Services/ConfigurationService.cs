using System;
using System.IO;
using System.Linq;
using System.Threading;
using ScheduledScreenshot.Models;

namespace ScheduledScreenshot.Services
{
    internal sealed class ConfigurationService : IDisposable
    {
        private readonly object _sync = new object();
        private readonly string _applicationDirectory;
        private readonly DiagnosticLogger _logger;
        private readonly SynchronizationContext _synchronizationContext;
        private readonly FileSystemWatcher _watcher;
        private Timer _debounceTimer;
        private AppSettings _current;
        private string _currentSignature;

        public ConfigurationService(
            string applicationDirectory,
            DiagnosticLogger logger,
            SynchronizationContext synchronizationContext)
        {
            _applicationDirectory = applicationDirectory;
            _logger = logger;
            _synchronizationContext = synchronizationContext ?? new SynchronizationContext();
            SettingsPath = Path.Combine(applicationDirectory, "settings.json");

            EnsureSettingsFile();
            LoadInitial();

            _watcher = new FileSystemWatcher(applicationDirectory, Path.GetFileName(SettingsPath))
            {
                NotifyFilter = NotifyFilters.LastWrite | NotifyFilters.FileName | NotifyFilters.Size
            };
            _watcher.Changed += OnFileChanged;
            _watcher.Created += OnFileChanged;
            _watcher.Renamed += OnFileChanged;
            _watcher.EnableRaisingEvents = true;
        }

        public string SettingsPath { get; }

        public AppSettings Current
        {
            get
            {
                lock (_sync)
                {
                    return Clone(_current);
                }
            }
        }

        public event Action<AppSettings> SettingsChanged;
        public event Action<string> SettingsRejected;

        public bool SetPaused(bool paused)
        {
            return Update(settings =>
            {
                settings.paused = paused;
                return true;
            });
        }

        public bool DisableTask(string taskId)
        {
            return Update(settings =>
            {
                var task = settings.tasks.FirstOrDefault(item =>
                    string.Equals(item.id, taskId, StringComparison.OrdinalIgnoreCase));
                if (task == null || !task.enabled)
                {
                    return false;
                }
                task.enabled = false;
                return true;
            });
        }

        public bool Update(Func<AppSettings, bool> mutation)
        {
            AppSettings updated;
            lock (_sync)
            {
                updated = Clone(_current);
                if (!mutation(updated))
                {
                    return false;
                }
                var validation = SettingsValidator.Validate(updated, _applicationDirectory, true);
                if (!validation.IsValid)
                {
                    PublishRejected(string.Join(Environment.NewLine, validation.Errors));
                    return false;
                }

                WriteAtomic(SettingsPath, JsonUtility.Serialize(updated, true));
                _current = updated;
                _currentSignature = JsonUtility.Signature(updated);
            }

            _logger.Info("CONFIG_UPDATED", "Configuration was updated by the application.", flush: true);
            PublishChanged(updated);
            return true;
        }

        public void Dispose()
        {
            _watcher?.Dispose();
            lock (_sync)
            {
                _debounceTimer?.Dispose();
                _debounceTimer = null;
            }
        }

        private void EnsureSettingsFile()
        {
            if (File.Exists(SettingsPath))
            {
                return;
            }
            var defaults = new AppSettings();
            defaults.tasks.Add(new ScreenshotTaskSettings { name = "Workday capture", enabled = false });
            WriteAtomic(SettingsPath, JsonUtility.Serialize(defaults, true));
        }

        private void LoadInitial()
        {
            try
            {
                var candidate = ReadSettingsFile();
                var validation = SettingsValidator.Validate(candidate, _applicationDirectory, true);
                if (!validation.IsValid)
                {
                    throw new InvalidDataException(string.Join(Environment.NewLine, validation.Errors));
                }
                _current = candidate;
                _currentSignature = JsonUtility.Signature(candidate);
                _logger.Configure(candidate.logging);
                _logger.Info("CONFIG_ACCEPTED", "Initial configuration was accepted.", flush: true);
            }
            catch (Exception exception)
            {
                _current = new AppSettings { paused = true };
                _currentSignature = JsonUtility.Signature(_current);
                _logger.Error("CONFIG_REJECTED", "Initial configuration is invalid; all tasks are disabled.", exception);
            }
        }

        private void OnFileChanged(object sender, FileSystemEventArgs arguments)
        {
            lock (_sync)
            {
                _debounceTimer?.Dispose();
                _debounceTimer = new Timer(_ => ReloadFromDisk(), null, 500, Timeout.Infinite);
            }
        }

        private void ReloadFromDisk()
        {
            try
            {
                var candidate = ReadSettingsFile();
                var validation = SettingsValidator.Validate(candidate, _applicationDirectory, true);
                if (!validation.IsValid)
                {
                    throw new InvalidDataException(string.Join(Environment.NewLine, validation.Errors));
                }
                var signature = JsonUtility.Signature(candidate);
                lock (_sync)
                {
                    if (signature == _currentSignature)
                    {
                        return;
                    }
                    _current = candidate;
                    _currentSignature = signature;
                }
                _logger.Configure(candidate.logging);
                _logger.Info("CONFIG_ACCEPTED", "Configuration changes were accepted.", flush: true);
                PublishChanged(candidate);
            }
            catch (Exception exception)
            {
                _logger.Error("CONFIG_REJECTED", "Configuration changes were rejected; the last valid settings remain active.", exception);
                PublishRejected(exception.Message);
            }
        }

        private AppSettings ReadSettingsFile()
        {
            using (var stream = new FileStream(SettingsPath, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete))
            using (var reader = new StreamReader(stream))
            {
                return JsonUtility.Deserialize<AppSettings>(reader.ReadToEnd());
            }
        }

        private void PublishChanged(AppSettings settings)
        {
            var copy = Clone(settings);
            _synchronizationContext.Post(_ => SettingsChanged?.Invoke(copy), null);
        }

        private void PublishRejected(string message)
        {
            _synchronizationContext.Post(_ => SettingsRejected?.Invoke(message), null);
        }

        private static AppSettings Clone(AppSettings settings)
        {
            return JsonUtility.Deserialize<AppSettings>(JsonUtility.Serialize(settings));
        }

        internal static void WriteAtomic(string path, string contents)
        {
            var directory = Path.GetDirectoryName(path);
            Directory.CreateDirectory(directory);
            var temporary = Path.Combine(directory,
                "." + Path.GetFileName(path) + "." + Guid.NewGuid().ToString("N") + ".tmp");
            try
            {
                using (var stream = new FileStream(temporary, FileMode.CreateNew, FileAccess.Write, FileShare.None,
                           4096, FileOptions.WriteThrough))
                using (var writer = new StreamWriter(stream, new System.Text.UTF8Encoding(false)))
                {
                    writer.Write(contents);
                    writer.Flush();
                    stream.Flush(true);
                }
                if (File.Exists(path))
                {
                    var backup = temporary + ".bak";
                    try
                    {
                        File.Replace(temporary, path, backup, true);
                        try { File.Delete(backup); } catch { }
                    }
                    catch (Exception exception) when (exception is PlatformNotSupportedException || exception is IOException)
                    {
                        File.Copy(temporary, path, true);
                        File.Delete(temporary);
                    }
                }
                else
                {
                    File.Move(temporary, path);
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
    }
}
