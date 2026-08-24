using System;
using Microsoft.Win32;

namespace ScheduledScreenshot.Services
{
    internal sealed class SessionMonitor : IDisposable
    {
        private bool _locked;
        private bool _suspended;

        public SessionMonitor()
        {
            SystemEvents.SessionSwitch += OnSessionSwitch;
            SystemEvents.PowerModeChanged += OnPowerModeChanged;
            SystemEvents.TimeChanged += OnTimeChanged;
        }

        public event Action<bool, string> AvailabilityChanged;
        public event Action ClockChanged;

        public void Dispose()
        {
            SystemEvents.SessionSwitch -= OnSessionSwitch;
            SystemEvents.PowerModeChanged -= OnPowerModeChanged;
            SystemEvents.TimeChanged -= OnTimeChanged;
        }

        private void OnSessionSwitch(object sender, SessionSwitchEventArgs arguments)
        {
            if (arguments.Reason == SessionSwitchReason.SessionLock
                || arguments.Reason == SessionSwitchReason.SessionLogoff
                || arguments.Reason == SessionSwitchReason.ConsoleDisconnect
                || arguments.Reason == SessionSwitchReason.RemoteDisconnect)
            {
                _locked = true;
                Publish("session locked or disconnected");
            }
            else if (arguments.Reason == SessionSwitchReason.SessionUnlock
                     || arguments.Reason == SessionSwitchReason.SessionLogon
                     || arguments.Reason == SessionSwitchReason.ConsoleConnect
                     || arguments.Reason == SessionSwitchReason.RemoteConnect)
            {
                _locked = false;
                Publish("session unlocked or connected");
            }
        }

        private void OnPowerModeChanged(object sender, PowerModeChangedEventArgs arguments)
        {
            if (arguments.Mode == PowerModes.Suspend)
            {
                _suspended = true;
                Publish("system suspended");
            }
            else if (arguments.Mode == PowerModes.Resume)
            {
                _suspended = false;
                Publish("system resumed");
            }
        }

        private void OnTimeChanged(object sender, EventArgs arguments)
        {
            ClockChanged?.Invoke();
        }

        private void Publish(string reason)
        {
            AvailabilityChanged?.Invoke(!_locked && !_suspended, reason);
        }
    }
}
