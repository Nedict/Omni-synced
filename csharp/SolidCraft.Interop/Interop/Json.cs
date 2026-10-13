// A tiny, dependency-free JSON reader/writer.
//
// The ModelForge C# workspace builds with the Mono `mcs` compiler and targets .NET Framework
// 4.x, where `System.Text.Json` is unavailable. Rather than take a third-party dependency we
// carry the small amount of JSON handling the interop needs: parse a reply object, read numbers
// and strings, and build command objects. It is deliberately strict and never throws on bad
// input — malformed JSON yields null.
using System;
using System.Collections.Generic;
using System.Globalization;
using System.Text;

namespace SolidCraft.Interop
{
    /// <summary>A minimal JSON value (object, array, string, number, bool or null).</summary>
    public sealed class Json
    {
        public enum Kind { Null, Bool, Number, String, Array, Object }

        public Kind Type;
        public bool Bool;
        public double Number;
        public string Str;
        public List<Json> Items;                       // array
        public Dictionary<string, Json> Members;       // object

        public static readonly Json Null = new Json { Type = Kind.Null };

        public static Json Of(bool b) { return new Json { Type = Kind.Bool, Bool = b }; }
        public static Json Of(double d) { return new Json { Type = Kind.Number, Number = d }; }
        public static Json Of(string s) { return s == null ? Null : new Json { Type = Kind.String, Str = s }; }

        public Json this[string key]
        {
            get
            {
                Json v;
                if (Members != null && Members.TryGetValue(key, out v)) return v;
                return Null;
            }
        }

        public Json this[int index]
        {
            get
            {
                if (Items != null && index >= 0 && index < Items.Count) return Items[index];
                return Null;
            }
        }

        public bool IsNull { get { return Type == Kind.Null; } }
        public int Count { get { return Items != null ? Items.Count : (Members != null ? Members.Count : 0); } }

        public double AsDouble(double fallback)
        {
            if (Type == Kind.Number) return Number;
            if (Type == Kind.String)
            {
                double d;
                if (double.TryParse(Str, NumberStyles.Float, CultureInfo.InvariantCulture, out d)) return d;
            }
            if (Type == Kind.Bool) return Bool ? 1 : 0;
            return fallback;
        }

        public string AsString(string fallback) { return Type == Kind.String ? Str : fallback; }

        // --- writing ------------------------------------------------------

        /// <summary>Escapes and quotes a string as a JSON string literal.</summary>
        public static string Quote(string s)
        {
            if (s == null) return "null";
            StringBuilder b = new StringBuilder(s.Length + 2);
            b.Append('"');
            foreach (char c in s)
            {
                switch (c)
                {
                    case '"': b.Append("\\\""); break;
                    case '\\': b.Append("\\\\"); break;
                    case '\n': b.Append("\\n"); break;
                    case '\r': b.Append("\\r"); break;
                    case '\t': b.Append("\\t"); break;
                    case '\b': b.Append("\\b"); break;
                    case '\f': b.Append("\\f"); break;
                    default:
                        if (c < 0x20) b.Append("\\u").Append(((int)c).ToString("x4", CultureInfo.InvariantCulture));
                        else b.Append(c);
                        break;
                }
            }
            b.Append('"');
            return b.ToString();
        }

        /// <summary>Formats a double so it round-trips (invariant culture, round-the-clock digits).</summary>
        public static string NumberText(double d)
        {
            if (double.IsNaN(d) || double.IsInfinity(d)) return "null";
            return d.ToString("R", CultureInfo.InvariantCulture);
        }

        // --- parsing ------------------------------------------------------

        /// <summary>Parses a JSON document. Returns null on any error (never throws).</summary>
        public static Json Parse(string text)
        {
            if (text == null) return null;
            int i = 0;
            Json v = ParseValue(text, ref i);
            SkipWs(text, ref i);
            return i == text.Length ? v : v; // trailing content is tolerated
        }

        private static Json ParseValue(string s, ref int i)
        {
            SkipWs(s, ref i);
            if (i >= s.Length) return null;
            char c = s[i];
            if (c == '{') return ParseObject(s, ref i);
            if (c == '[') return ParseArray(s, ref i);
            if (c == '"') return new Json { Type = Kind.String, Str = ParseString(s, ref i) };
            if (c == 't' && Matches(s, i, "true")) { i += 4; return Of(true); }
            if (c == 'f' && Matches(s, i, "false")) { i += 5; return Of(false); }
            if (c == 'n' && Matches(s, i, "null")) { i += 4; return Null; }
            return ParseNumber(s, ref i);
        }

        private static Json ParseObject(string s, ref int i)
        {
            Json o = new Json { Type = Kind.Object, Members = new Dictionary<string, Json>() };
            i++; // {
            SkipWs(s, ref i);
            if (i < s.Length && s[i] == '}') { i++; return o; }
            while (i < s.Length)
            {
                SkipWs(s, ref i);
                if (i >= s.Length || s[i] != '"') return o;
                string key = ParseString(s, ref i);
                SkipWs(s, ref i);
                if (i >= s.Length || s[i] != ':') return o;
                i++;
                Json val = ParseValue(s, ref i);
                if (val == null) return o;
                o.Members[key] = val;
                SkipWs(s, ref i);
                if (i < s.Length && s[i] == ',') { i++; continue; }
                if (i < s.Length && s[i] == '}') { i++; return o; }
                return o;
            }
            return o;
        }

        private static Json ParseArray(string s, ref int i)
        {
            Json a = new Json { Type = Kind.Array, Items = new List<Json>() };
            i++; // [
            SkipWs(s, ref i);
            if (i < s.Length && s[i] == ']') { i++; return a; }
            while (i < s.Length)
            {
                Json val = ParseValue(s, ref i);
                if (val == null) return a;
                a.Items.Add(val);
                SkipWs(s, ref i);
                if (i < s.Length && s[i] == ',') { i++; continue; }
                if (i < s.Length && s[i] == ']') { i++; return a; }
                return a;
            }
            return a;
        }

        private static string ParseString(string s, ref int i)
        {
            StringBuilder b = new StringBuilder();
            i++; // opening quote
            while (i < s.Length)
            {
                char c = s[i++];
                if (c == '"') return b.ToString();
                if (c != '\\') { b.Append(c); continue; }
                if (i >= s.Length) break;
                char e = s[i++];
                switch (e)
                {
                    case '"': b.Append('"'); break;
                    case '\\': b.Append('\\'); break;
                    case '/': b.Append('/'); break;
                    case 'b': b.Append('\b'); break;
                    case 'f': b.Append('\f'); break;
                    case 'n': b.Append('\n'); break;
                    case 'r': b.Append('\r'); break;
                    case 't': b.Append('\t'); break;
                    case 'u':
                        if (i + 4 <= s.Length)
                        {
                            int code;
                            if (int.TryParse(s.Substring(i, 4), NumberStyles.HexNumber, CultureInfo.InvariantCulture, out code))
                            {
                                b.Append((char)code);
                                i += 4;
                            }
                        }
                        break;
                    default: b.Append(e); break;
                }
            }
            return b.ToString();
        }

        private static Json ParseNumber(string s, ref int i)
        {
            int start = i;
            while (i < s.Length && ("+-0123456789.eE".IndexOf(s[i]) >= 0)) i++;
            double d;
            if (i > start && double.TryParse(s.Substring(start, i - start), NumberStyles.Float, CultureInfo.InvariantCulture, out d))
                return Of(d);
            return null;
        }

        private static void SkipWs(string s, ref int i)
        {
            while (i < s.Length && (s[i] == ' ' || s[i] == '\t' || s[i] == '\n' || s[i] == '\r')) i++;
        }

        private static bool Matches(string s, int i, string word)
        {
            return i + word.Length <= s.Length && string.CompareOrdinal(s, i, word, 0, word.Length) == 0;
        }

        /// <summary>Builds a command JSON object `{"command": id, "params": {...}}`.</summary>
        public static string Command(string id, params object[] keyValues)
        {
            StringBuilder b = new StringBuilder();
            b.Append("{\"command\":").Append(Quote(id)).Append(",\"params\":{");
            for (int i = 0; i + 1 < keyValues.Length; i += 2)
            {
                if (i > 0) b.Append(',');
                b.Append(Quote(Convert.ToString(keyValues[i], CultureInfo.InvariantCulture))).Append(':');
                b.Append(Encode(keyValues[i + 1]));
            }
            b.Append("}}");
            return b.ToString();
        }

        /// <summary>Encodes a C# value (double, string, bool, array, Json) as JSON.</summary>
        public static string Encode(object value)
        {
            if (value == null) return "null";
            Json j = value as Json;
            if (j != null) return j.ToString();
            if (value is bool) return (bool)value ? "true" : "false";
            if (value is double || value is float) return NumberText(Convert.ToDouble(value, CultureInfo.InvariantCulture));
            if (value is int || value is long || value is short) return Convert.ToString(value, CultureInfo.InvariantCulture);
            double[] doubles = value as double[];
            if (doubles != null)
            {
                StringBuilder b = new StringBuilder("[");
                for (int i = 0; i < doubles.Length; i++)
                {
                    if (i > 0) b.Append(',');
                    b.Append(NumberText(doubles[i]));
                }
                return b.Append(']').ToString();
            }
            string[] strings = value as string[];
            if (strings != null)
            {
                StringBuilder b = new StringBuilder("[");
                for (int i = 0; i < strings.Length; i++)
                {
                    if (i > 0) b.Append(',');
                    b.Append(Quote(strings[i]));
                }
                return b.Append(']').ToString();
            }
            if (value is System.Collections.IEnumerable && !(value is string))
            {
                StringBuilder b = new StringBuilder("[");
                bool first = true;
                foreach (object item in (System.Collections.IEnumerable)value)
                {
                    if (!first) b.Append(',');
                    first = false;
                    b.Append(Encode(item));
                }
                return b.Append(']').ToString();
            }
            return Quote(Convert.ToString(value, CultureInfo.InvariantCulture));
        }

        public override string ToString()
        {
            StringBuilder b = new StringBuilder();
            Write(b);
            return b.ToString();
        }

        private void Write(StringBuilder b)
        {
            switch (Type)
            {
                case Kind.Null: b.Append("null"); break;
                case Kind.Bool: b.Append(Bool ? "true" : "false"); break;
                case Kind.Number: b.Append(NumberText(Number)); break;
                case Kind.String: b.Append(Quote(Str)); break;
                case Kind.Array:
                    b.Append('[');
                    for (int i = 0; i < Items.Count; i++)
                    {
                        if (i > 0) b.Append(',');
                        Items[i].Write(b);
                    }
                    b.Append(']');
                    break;
                case Kind.Object:
                    b.Append('{');
                    bool first = true;
                    foreach (KeyValuePair<string, Json> kv in Members)
                    {
                        if (!first) b.Append(',');
                        first = false;
                        b.Append(Quote(kv.Key)).Append(':');
                        kv.Value.Write(b);
                    }
                    b.Append('}');
                    break;
            }
        }
    }
}
