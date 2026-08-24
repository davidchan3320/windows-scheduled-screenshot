using System;
using System.Runtime.InteropServices;
using System.Threading;
using System.Windows.Forms;

namespace ScheduledScreenshot
{
    internal static class Program
    {
        private const string MutexName = @"Local\ScheduledScreenshot-7C7386A0-488B-4EB1-A0B2-A8D4071149AF";

        [STAThread]
        private static void Main()
        {
            bool ownsMutex;
            using (var mutex = new Mutex(true, MutexName, out ownsMutex))
            {
                if (!ownsMutex)
                {
                    MessageBox.Show("Scheduled Screenshot is already running.", "Scheduled Screenshot",
                        MessageBoxButtons.OK, MessageBoxIcon.Information);
                    return;
                }

                EnableDpiAwareness();
                Application.EnableVisualStyles();
                Application.SetCompatibleTextRenderingDefault(false);
                SynchronizationContext.SetSynchronizationContext(new WindowsFormsSynchronizationContext());
                Application.Run(new TrayApplicationContext());
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
