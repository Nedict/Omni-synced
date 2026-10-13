// Application — the C# implementation of the SolidWorks application object (`ISldWorks`).
//
// Where SolidWorks attaches to a COM server, this holds documents in-process, each owning a
// SolidCraft session. The workspace's `ModelForgeAdapter.ActiveApplication()` obtains one of these
// once per process (see Attach), then creates and opens documents through it.
using System;
using System.Collections.Generic;
using System.IO;
using SolidCraft.Interop;

namespace ModelForge.Interop.sldworks
{
    /// <summary>A SolidCraft-backed SolidWorks application.</summary>
    public sealed class Application : ISldWorks
    {
        private readonly Dictionary<string, Document> _documents = new Dictionary<string, Document>(StringComparer.OrdinalIgnoreCase);
        private int _partSerial;
        private static Application _active;

        public Application() { }

        /// <summary>
        /// The process-wide application, created on first use — the SolidCraft stand-in for
        /// `Marshal.GetActiveObject("SldWorks.Application")`. Returns the same instance every call,
        /// so a host that attaches twice talks to one set of documents.
        /// </summary>
        public static Application Attach()
        {
            if (_active == null) _active = new Application();
            return _active;
        }

        /// <summary>Releases the process-wide application (tests).</summary>
        public static void Detach()
        {
            if (_active == null) return;
            foreach (Document d in new List<Document>(_active._documents.Values)) d.Dispose();
            _active._documents.Clear();
            _active = null;
        }

        // --- ISldWorks ------------------------------------------------

        public IModelDoc2 NewPart()
        {
            string title = "Part" + (++_partSerial);
            Document doc = new Document(this, DocumentKind.Part, title);
            _documents[title] = doc;
            return doc;
        }

        public IModelDoc2 NewAssembly()
        {
            string title = "Assembly" + (++_partSerial);
            Document doc = new Document(this, DocumentKind.Assembly, title);
            _documents[title] = doc;
            return doc;
        }

        public IModelDoc2 NewDocument(string templateName, int paperSize, double width, double height)
        {
            // The template names a part/assembly/drawing; the kind is inferred from the extension.
            string lower = (templateName ?? "").ToLowerInvariant();
            if (lower.EndsWith(".slddrw")) return OpenNew(DocumentKind.Drawing);
            if (lower.EndsWith(".sldasm")) return NewAssembly();
            return NewPart();
        }

        private Document OpenNew(DocumentKind kind)
        {
            string title = kind + (++_partSerial).ToString();
            Document doc = new Document(this, kind, title);
            _documents[title] = doc;
            return doc;
        }

        public IModelDoc2 OpenDoc6(string fileName, int type, int options, string configuration, ref int errors, ref int warnings)
        {
            errors = 0;
            warnings = 0;
            if (string.IsNullOrEmpty(fileName)) { errors = 1; return null; }
            string title = Path.GetFileName(fileName);
            Document existing;
            if (_documents.TryGetValue(title, out existing)) return existing;
            DocumentKind kind = type == 2 ? DocumentKind.Assembly : type == 3 ? DocumentKind.Drawing : DocumentKind.Part;
            Document doc = new Document(this, kind, title);
            if (File.Exists(fileName))
            {
                string json = File.ReadAllText(fileName);
                string error = doc.LoadDocument(json);
                if (error != null)
                {
                    // Not a SolidCraft design: try importing it as STEP/other through the engine.
                    Json r = doc.Raw.TryExecute("doc.open", "path", fileName);
                    if (r == null) { errors = 1; doc.Dispose(); return null; }
                }
                doc.SetPath(fileName);
            }
            _documents[title] = doc;
            return doc;
        }

        public IModelDoc2 ActivateDoc2(string name, bool silent, ref int errors)
        {
            errors = 0;
            Document doc;
            if (name != null && _documents.TryGetValue(Path.GetFileName(name), out doc)) return doc;
            return null;
        }

        public bool CloseDoc(string name)
        {
            Document doc;
            if (name != null && _documents.TryGetValue(Path.GetFileName(name), out doc))
            {
                doc.Dispose();
                _documents.Remove(Path.GetFileName(name));
                return true;
            }
            return false;
        }

        public IModelDoc2 GetOpenDocumentByName(string name)
        {
            Document doc;
            if (name != null && _documents.TryGetValue(Path.GetFileName(name), out doc)) return doc;
            return null;
        }

        public object GetDocumentDependencies2(string document, bool traverse, bool searchAssemblies, bool addReadOnlyInfo)
        {
            return new object[0];
        }

        public bool ReplaceReferencedDocument(string document, string oldName, string newName)
        {
            // A single-document workspace has no external references to remap.
            return true;
        }

        public int GetProcessID() { return System.Diagnostics.Process.GetCurrentProcess().Id; }

        public IMathUtility GetMathUtility() { return new MathUtility(); }

        public bool GetUserPreferenceToggle(int userPreference) { return false; }
        public void SetUserPreferenceToggle(int userPreference, bool option) { }
        public string GetUserPreferenceStringValue(int userPreference) { return ""; }

        // --- helpers -------------------------------------------------

        internal bool SaveDocument(Document doc)
        {
            string path = doc.GetPathName();
            if (string.IsNullOrEmpty(path)) return false;
            File.WriteAllText(path, doc.DocumentJson);
            return true;
        }
    }

    /// <summary>The math utility: point and vector factories.</summary>
    internal sealed class MathUtility : IMathUtility
    {
        public IMathPoint CreatePoint(object point)
        {
            double[] values = point as double[];
            IMathTransform t = point as IMathTransform;
            double[] p = values ?? new double[] { 0, 0, 0 };
            return new MathPoint(p);
        }

        public IMathVector CreateVector(object vector)
        {
            double[] values = vector as double[];
            return new MathVector(values ?? new double[] { 0, 0, 0 });
        }
    }

    internal sealed class MathPoint : IMathPoint
    {
        private object[] _data;
        public MathPoint(double[] values)
        {
            _data = new object[] { values.Length > 0 ? values[0] : 0.0, values.Length > 1 ? values[1] : 0.0, values.Length > 2 ? values[2] : 0.0 };
        }
        public object[] ArrayData { get { return _data; } set { _data = value; } }
        public IMathPoint MultiplyTransform(IMathTransform transform) { return this; }
    }

    internal sealed class MathVector : IMathVector
    {
        private object[] _data;
        public MathVector(double[] values)
        {
            _data = new object[] { values.Length > 0 ? values[0] : 0.0, values.Length > 1 ? values[1] : 0.0, values.Length > 2 ? values[2] : 0.0 };
        }
        public object[] ArrayData { get { return _data; } set { _data = value; } }
        public IMathVector MultiplyTransform(IMathTransform transform) { return this; }
    }
}
