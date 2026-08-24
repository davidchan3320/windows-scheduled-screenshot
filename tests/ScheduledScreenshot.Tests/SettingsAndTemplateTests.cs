using System;
using System.IO;
using System.Linq;
using Microsoft.VisualStudio.TestTools.UnitTesting;
using ScheduledScreenshot.Models;
using ScheduledScreenshot.Services;

namespace ScheduledScreenshot.Tests
{
    [TestClass]
    public sealed class SettingsAndTemplateTests
    {
        [TestMethod]
        public void DefaultSettingsAreValid()
        {
            var settings = new AppSettings();
            settings.tasks.Add(new ScreenshotTaskSettings { enabled = false });

            var result = SettingsValidator.Validate(settings, Path.GetTempPath(), false);

            Assert.IsTrue(result.IsValid, string.Join(Environment.NewLine, result.Errors));
        }

        [TestMethod]
        public void FilenameTemplateRequiresDisplayToken()
        {
            var errors = FileNameTemplate.Validate("{timestamp}_{task}").ToList();

            Assert.IsTrue(errors.Any(error => error.Contains("{display}")));
        }

        [TestMethod]
        public void FilenameTemplateExpandsTaskAndDisplayValues()
        {
            var task = new ScreenshotTaskSettings
            {
                id = "491e5e22-748b-41c8-9df6-4a5b816f8c02",
                name = "Daily: report"
            };

            var actual = FileNameTemplate.Expand("{date}_{task}_{displayIndex}", task,
                new DateTime(2026, 8, 24, 13, 59, 1, 42), @"\\.\DISPLAY1", 2);

            Assert.AreEqual("20260824_Daily_ report_2", actual);
        }

        [TestMethod]
        public void InvalidLoggingTraversalIsRejected()
        {
            var settings = new AppSettings();
            settings.logging.directory = @"..\outside";

            var result = SettingsValidator.Validate(settings, Path.GetTempPath(), false);

            Assert.IsFalse(result.IsValid);
            Assert.IsTrue(result.Errors.Any(error => error.Contains("logging.directory")));
        }
    }
}
