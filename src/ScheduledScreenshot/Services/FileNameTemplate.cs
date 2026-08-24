using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Text;
using System.Text.RegularExpressions;
using ScheduledScreenshot.Models;

namespace ScheduledScreenshot.Services
{
    internal static class FileNameTemplate
    {
        private static readonly HashSet<string> Tokens = new HashSet<string>(StringComparer.OrdinalIgnoreCase)
        {
            "timestamp", "date", "time", "task", "taskId", "taskId8", "display", "displayIndex"
        };

        private static readonly Regex TokenPattern = new Regex(@"\{([A-Za-z0-9]+)\}", RegexOptions.Compiled);
        private static readonly HashSet<string> ReservedNames = new HashSet<string>(StringComparer.OrdinalIgnoreCase)
        {
            "CON", "PRN", "AUX", "NUL",
            "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9",
            "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9"
        };

        public static IEnumerable<string> Validate(string template)
        {
            if (string.IsNullOrWhiteSpace(template))
            {
                yield return "Filename template is required.";
                yield break;
            }

            if (template.Length > 180)
            {
                yield return "Filename template cannot exceed 180 characters.";
            }

            if (template.IndexOfAny(new[] { '\\', '/', ':', '<', '>', '"', '|', '?', '*' }) >= 0)
            {
                yield return "Filename template contains an invalid Windows filename character or path separator.";
            }

            foreach (Match match in TokenPattern.Matches(template))
            {
                if (!Tokens.Contains(match.Groups[1].Value))
                {
                    yield return "Unknown filename token: " + match.Value + ".";
                }
            }

            var withoutTokens = TokenPattern.Replace(template, string.Empty);
            if (withoutTokens.Contains("{") || withoutTokens.Contains("}"))
            {
                yield return "Filename template contains a malformed token.";
            }

            if (!ContainsToken(template, "display") && !ContainsToken(template, "displayIndex"))
            {
                yield return "Filename template must include {display} or {displayIndex}.";
            }
        }

        public static string ResolveTaskSpecificTemplate(string template, ScreenshotTaskSettings task)
        {
            var taskId = ParseTaskId(task.id);
            return ReplaceToken(ReplaceToken(ReplaceToken(template, "task", Sanitize(task.name)),
                "taskId", taskId), "taskId8", taskId.Replace("-", string.Empty).Substring(0, 8));
        }

        public static string Expand(
            string template,
            ScreenshotTaskSettings task,
            DateTime localTimestamp,
            string displayName,
            int displayIndex)
        {
            var taskId = ParseTaskId(task.id);
            var values = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase)
            {
                ["timestamp"] = localTimestamp.ToString("yyyyMMdd_HHmmss_fff", CultureInfo.InvariantCulture),
                ["date"] = localTimestamp.ToString("yyyyMMdd", CultureInfo.InvariantCulture),
                ["time"] = localTimestamp.ToString("HHmmss", CultureInfo.InvariantCulture),
                ["task"] = Sanitize(task.name),
                ["taskId"] = taskId,
                ["taskId8"] = taskId.Replace("-", string.Empty).Substring(0, 8),
                ["display"] = Sanitize(NormalizeDisplayName(displayName)),
                ["displayIndex"] = displayIndex.ToString(CultureInfo.InvariantCulture)
            };

            var expanded = TokenPattern.Replace(template, match => values[match.Groups[1].Value]);
            expanded = expanded.Trim().TrimEnd('.', ' ');
            if (expanded.Length == 0)
            {
                expanded = "screenshot";
            }

            var stem = Path.GetFileNameWithoutExtension(expanded);
            if (ReservedNames.Contains(stem))
            {
                expanded = "_" + expanded;
            }
            return expanded;
        }

        public static string Sanitize(string value)
        {
            if (string.IsNullOrWhiteSpace(value))
            {
                return "unnamed";
            }

            var invalid = new HashSet<char>(Path.GetInvalidFileNameChars().Concat(new[]
            {
                '\\', '/', ':', '<', '>', '"', '|', '?', '*'
            }));
            var builder = new StringBuilder(value.Length);
            foreach (var character in value)
            {
                builder.Append(invalid.Contains(character) || char.IsControl(character) ? '_' : character);
            }

            var result = builder.ToString().Trim().TrimEnd('.', ' ');
            return result.Length == 0 ? "unnamed" : result;
        }

        private static bool ContainsToken(string template, string token)
        {
            return template.IndexOf("{" + token + "}", StringComparison.OrdinalIgnoreCase) >= 0;
        }

        private static string ReplaceToken(string template, string token, string value)
        {
            return Regex.Replace(template, "\\{" + Regex.Escape(token) + "\\}",
                _ => value, RegexOptions.IgnoreCase);
        }

        private static string ParseTaskId(string id)
        {
            return Guid.TryParse(id, out var parsed) ? parsed.ToString() : Guid.Empty.ToString();
        }

        private static string NormalizeDisplayName(string value)
        {
            if (value != null && value.StartsWith(@"\\.\", StringComparison.Ordinal))
            {
                return value.Substring(4);
            }
            return value;
        }
    }
}
