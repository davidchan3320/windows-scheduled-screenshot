using System;
using System.Security.Cryptography;
using System.Text;
using System.Web.Script.Serialization;

namespace ScheduledScreenshot.Services
{
    internal static class JsonUtility
    {
        private static JavaScriptSerializer CreateSerializer()
        {
            return new JavaScriptSerializer
            {
                MaxJsonLength = 4 * 1024 * 1024,
                RecursionLimit = 128
            };
        }

        public static T Deserialize<T>(string json)
        {
            return CreateSerializer().Deserialize<T>(json);
        }

        public static string Serialize(object value, bool indented = false)
        {
            var json = CreateSerializer().Serialize(value);
            return indented ? PrettyPrint(json) : json;
        }

        public static string Signature(object value)
        {
            using (var sha = SHA256.Create())
            {
                var bytes = Encoding.UTF8.GetBytes(Serialize(value));
                return Convert.ToBase64String(sha.ComputeHash(bytes));
            }
        }

        public static string PrettyPrint(string json)
        {
            var result = new StringBuilder(json.Length + 256);
            var indent = 0;
            var quoted = false;
            var escaped = false;

            foreach (var character in json)
            {
                if (quoted)
                {
                    result.Append(character);
                    if (escaped)
                    {
                        escaped = false;
                    }
                    else if (character == '\\')
                    {
                        escaped = true;
                    }
                    else if (character == '"')
                    {
                        quoted = false;
                    }
                    continue;
                }

                switch (character)
                {
                    case '"':
                        quoted = true;
                        result.Append(character);
                        break;
                    case '{':
                    case '[':
                        result.Append(character).AppendLine();
                        indent++;
                        AppendIndent(result, indent);
                        break;
                    case '}':
                    case ']':
                        result.AppendLine();
                        indent--;
                        AppendIndent(result, indent);
                        result.Append(character);
                        break;
                    case ',':
                        result.Append(character).AppendLine();
                        AppendIndent(result, indent);
                        break;
                    case ':':
                        result.Append(": ");
                        break;
                    default:
                        if (!char.IsWhiteSpace(character))
                        {
                            result.Append(character);
                        }
                        break;
                }
            }

            return result.ToString();
        }

        private static void AppendIndent(StringBuilder builder, int count)
        {
            builder.Append(' ', Math.Max(0, count) * 2);
        }
    }
}
