using System;
using System.Runtime.InteropServices;
using System.Threading;
using System.Windows.Forms;

namespace ScheduledScreenshot
{
    internal static class Program
    {
        private const string MutexName = @"Local\ScheduledScreenshot-7C7386A0-488B-4EB1-A0B2-A8D4071149AF";
        private const string SmokeTestArgument = "--smoke-test";

        [STAThread]
        private static int Main(string[] arguments)
        {
            var smokeTest = Array.Exists(arguments ?? Array.Empty<string>(), argument =>
                string.Equals(argument, SmokeTestArgument, StringComparison.OrdinalIgnoreCase));
            bool ownsMutex;
            using (var mutex = new Mutex(true, MutexName, out ownsMutex))
            {
                if (!ownsMutex)
                {
                    if (!smokeTest)
                    {
                        MessageBox.Show("Scheduled Screenshot is already running.", "Scheduled Screenshot",
                            MessageBoxButtons.OK, MessageBoxIcon.Information);
                    }
                    return smokeTest ? 2 : 0;
                }

                EnableDpiAwareness();
                Application.EnableVisualStyles();
                Application.SetCompatibleTextRenderingDefault(false);
                SynchronizationContext.SetSynchronizationContext(new WindowsFormsSynchronizationContext());
                var context = new TrayApplicationContext();
                System.Windows.Forms.Timer smokeTimer = null;
                if (smokeTest)
                {
                    smokeTimer = new System.Windows.Forms.Timer { Interval = 8000 };
                    smokeTimer.Tick += (sender, eventArguments) =>
                    {
                        smokeTimer.Stop();
                        context.ExitApplication();
                    };
                    smokeTimer.Start();
                }

                Application.Run(context);
                smokeTimer?.Dispose();
                return 0;
            }
        }

        private static void EnableDpiAwareness()
        {
            try
            {
                SetProcessDpiAwarenessContext(new IntPtr(-4));
            }
            catch (EntryPointNotFoundException)
            {
                SetProcessDPIAware();
            }
        }

        [DllImport("user32.dll")]
        [return: MarshalAs(UnmanagedType.Bool)]
        private static extern bool SetProcessDpiAwarenessContext(IntPtr value);

        [DllImport("user32.dll")]
        [return: MarshalAs(UnmanagedType.Bool)]
        private static extern bool SetProcessDPIAware();
    }
}
