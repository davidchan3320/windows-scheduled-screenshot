using System;
using System.IO;
using ScheduledScreenshot.Models;

namespace ScheduledScreenshot.Services
{
    internal sealed class RuntimeStateStore
    {
        private readonly object _sync = new object();
        private readonly string _path;
        private readonly DiagnosticLogger _logger;
        private RuntimeState _state;

        public RuntimeStateStore(string applicationDirectory, DiagnosticLogger logger)
        {
            _path = Path.Combine(applicationDirectory, "runtime-state.json");
            _logger = logger;
            _state = Load();
        }

        public RuntimeState Snapshot()
        {
            lock (_sync)
            {
                return Clone(_state);
            }
        }

        public void Mutate(Action<RuntimeState> mutation)
        {
            lock (_sync)
            {
                mutation(_state);
                Save();
            }
        }

        private RuntimeState Load()
        {
            if (!File.Exists(_path))
            {
                return new RuntimeState();
            }
            try
            {
                var state = JsonUtility.Deserialize<RuntimeState>(File.ReadAllText(_path));
                return state ?? new RuntimeState();
            }
            catch (Exception exception)
            {
                _logger.Error("RUNTIME_STATE_REJECTED", "Runtime state could not be read; safe new state will be used.", exception);
                return new RuntimeState();
            }
        }

        private void Save()
        {
            try
            {
                ConfigurationService.WriteAtomic(_path, JsonUtility.Serialize(_state, true));
            }
            catch (Exception exception)
            {
                _logger.Error("RUNTIME_STATE_WRITE_FAILED", "Runtime state could not be persisted.", exception);
            }
        }

        private static RuntimeState Clone(RuntimeState state)
        {
            return JsonUtility.Deserialize<RuntimeState>(JsonUtility.Serialize(state));
        }
    }
}
