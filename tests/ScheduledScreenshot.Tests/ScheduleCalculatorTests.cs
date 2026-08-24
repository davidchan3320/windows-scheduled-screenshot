using System;
using Microsoft.VisualStudio.TestTools.UnitTesting;
using ScheduledScreenshot.Models;
using ScheduledScreenshot.Services;

namespace ScheduledScreenshot.Tests
{
    [TestClass]
    public sealed class ScheduleCalculatorTests
    {
        [TestMethod]
        public void OneSecondIntervalReturnsNextCadencePoint()
        {
            var now = new DateTimeOffset(2026, 8, 24, 6, 0, 0, TimeSpan.Zero);
            var task = EnabledIntervalTask(1);
            var state = new TaskRuntimeState { intervalAnchorUtc = now.ToString("o") };

            var result = ScheduleCalculator.GetNextOccurrence(task, state, now);

            Assert.IsNotNull(result);
            Assert.AreEqual(now.AddSeconds(1), result.DueUtc);
        }

        [TestMethod]
        public void StopConditionWinsOverOccurrenceAtSameTime()
        {
            var now = new DateTimeOffset(2026, 8, 24, 6, 0, 0, TimeSpan.Zero);
            var task = EnabledIntervalTask(1);
            task.stopCondition.mode = "duration";
            task.stopCondition.durationSeconds = 1;
            var state = new TaskRuntimeState
            {
                intervalAnchorUtc = now.ToString("o"),
                durationDeadlineUtc = now.AddSeconds(1).ToString("o")
            };

            var result = ScheduleCalculator.GetNextOccurrence(task, state, now);

            Assert.IsNull(result);
        }

        [TestMethod]
        public void DisabledTaskHasNoOccurrence()
        {
            var task = EnabledIntervalTask(60);
            task.enabled = false;

            var result = ScheduleCalculator.GetNextOccurrence(task,
                new TaskRuntimeState { intervalAnchorUtc = DateTimeOffset.UtcNow.ToString("o") }, DateTimeOffset.UtcNow);

            Assert.IsNull(result);
        }

        private static ScreenshotTaskSettings EnabledIntervalTask(int seconds)
        {
            return new ScreenshotTaskSettings
            {
                enabled = true,
                schedule = new ScheduleSettings
                {
                    type = "interval",
                    intervalSeconds = seconds,
                    activeHoursEnabled = false
                }
            };
        }
    }
}
