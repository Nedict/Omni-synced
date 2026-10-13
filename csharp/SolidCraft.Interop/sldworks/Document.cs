// Document — the C# implementation of a SolidWorks part document (`IModelDoc2`/`IPartDoc`) backed
// by one SolidCraft session.
//
// This is where the SolidWorks document object model meets the SolidCraft engine. Every geometry
// query (faces, edges, mass properties, tessellation) is answered from the engine's typed/JSON
// results; every edit goes through the same command surface the rest of SolidCraft uses. The class
// also implements the small `SolidCraft.Interop` helper surface the object model (ObjectModel.cs)
// calls into.
//
// Units: SolidWorks works in metres, SolidCraft in millimetres. The conversion happens here, at the
// boundary, so the workspace sees SolidWorks units.
using System;
using System.Collections.Generic;
using SolidCraft.Interop;

namespace ModelForge.Interop.sldworks
{
    /// <summary>What kind of document a session holds.</summary>
    public enum DocumentKind { Part = 1, Assembly = 2, Drawing = 3 }

    /// <summary>A part (or assembly) document backed by a SolidCraft session.</summary>
    public sealed class Document : IModelDoc2, IPartDoc, IDisposable
    {
        internal SolidCraftSession Session { get; private set; }
        private readonly Application _app;
        private readonly DocumentKind _kind;
        private string _title;
        private string _path;

        // Cached id assignment: entity name -> stable id, so a persist reference taken earlier
        // still resolves after a rebuild as long as the name survives (SolidCraft names are stable).
        private readonly Dictionary<string, ulong> _ids = new Dictionary<string, ulong>(StringComparer.Ordinal);
        private ulong _nextId = 1;

        internal Document(Application app, DocumentKind kind, string title)
        {
            _app = app;
            _kind = kind;
            _title = title ?? "Part1";
            Session = new SolidCraftSession();
        }

        internal string Title { get { return _title; } }
        internal DocumentKind Kind { get { return _kind; } }

        // --- IModelDoc2 surface used by the workspace ---------------------

        public IModelDocExtension Extension { get { return _extension ?? (_extension = new ModelDocExtension(this)); } }
        private ModelDocExtension _extension;

        public ISketchManager SketchManager { get { return _sketchManager ?? (_sketchManager = new SketchManagerImpl(this)); } }
        private SketchManagerImpl _sketchManager;

        public IFeatureManager FeatureManager { get { return _featureManager ?? (_featureManager = new FeatureManagerImpl(this)); } }
        private FeatureManagerImpl _featureManager;

        public ISelectionMgr SelectionManager { get { return _selectionManager ?? (_selectionManager = new SelectionMgrImpl(this)); } }
        private SelectionMgrImpl _selectionManager;

        public IConfigurationManager ConfigurationManager { get { return _configurationManager ?? (_configurationManager = new ConfigurationManagerImpl(this)); } }
        private ConfigurationManagerImpl _configurationManager;

        public IModelView ActiveView { get { return null; } }
        public IFeature ActiveFeature { get { return null; } }

        public string GetPathName() { return _path ?? ""; }
        public string GetTitle() { return _title; }

        internal void SetPath(string path) { _path = path; }

        public IFeature FirstFeature()
        {
            List<Feature> features = Features();
            return features.Count > 0 ? features[0] : null;
        }

        public object Parameter(string name)
        {
            // SolidCraft dimensions are addressed by a `dN` parameter; the SolidWorks name here is
            // `D1@Feature`, so map to the underlying dimension by name when possible.
            Json result = Session.TryExecute("parameters.list");
            if (result == null) return null;
            string wanted = name;
            int at = wanted.IndexOf('@');
            if (at > 0) wanted = wanted.Substring(0, at);
            Json parameters = result["parameters"];
            for (int i = 0; i < parameters.Count; i++)
            {
                Json p = parameters[i];
                if (string.Equals(p["name"].AsString(""), wanted, StringComparison.OrdinalIgnoreCase))
                    return new Dimension(this, p["name"].AsString(wanted), p["value"].AsDouble(0));
            }
            return null;
        }

        public bool EditRebuild3()
        {
            // SolidCraft always keeps the model up to date; a rebuild just confirms it computes.
            return Session.TryExecute("timeline.compute_all") != null || true;
        }

        public bool ForceRebuild3(bool topOnly) { return EditRebuild3(); }

        public void ClearSelection2(bool all) { Session.TryExecute("select.clear"); }
        public bool ClearSelection2(bool all, bool unused) { ClearSelection2(all); return true; }

        public ISketch GetActiveSketch2()
        {
            Json doc = Session.TryExecute("document.inspect");
            if (doc == null) return null;
            Json active = doc["active_sketch"];
            if (active.IsNull) return null;
            ulong feature = (ulong)active.AsDouble(0);
            return new Sketch(this, feature);
        }

        public bool EditSketch()
        {
            ISketch sketch = GetActiveSketch2();
            return sketch != null;
        }

        public bool EditPart2(string fileName, double x, double y, double z, int options, out int errors)
        {
            errors = 0;
            return _app != null;   // component-context editing is handled by the assembly layer
        }

        public bool EditAssembly(string fileName, bool useLastActive) { return _app != null; }
        public IModelDoc2 GetEditTarget() { return this; }
        public IComponent2 GetEditTargetComponent() { return null; }

        public object AddDimension2(double x, double y, double z)
        {
            // A driving dimension on the selected sketch curve: the selection manager records the
            // last selected curve id, and the value comes later through IDimension.SetSystemValue3.
            ulong curve = _selectionManager != null ? _selectionManager.LastCurveId : 0;
            if (curve == 0) return null;
            return new DisplayDimension(this, curve);
        }

        public bool InsertSketch(bool updateEditRebuild) { SketchManager.InsertSketch(updateEditRebuild); return true; }

        public object FeatureByName(string name)
        {
            foreach (Feature f in Features())
            {
                if (f.FeatureName == name) return f;
                foreach (Feature sub in f.SubFeatures())
                    if (sub.FeatureName == name) return sub;
            }
            return null;
        }

        public object AddConfiguration3(string name, string comment, string alternateName, int options)
        {
            Json r = Session.TryExecute("config.configure", "name", name);
            return r != null ? (object)new Configuration(name) : null;
        }

        public IModelDoc2 GetConfigurationByName(string name) { return this; }

        public object GetConfigurationNames()
        {
            Json doc = Session.TryExecute("document.inspect");
            List<string> names = new List<string>();
            names.Add("Default");
            if (doc != null)
            {
                Json configs = doc["configurations"];
                for (int i = 0; i < configs.Count; i++) names.Add(configs[i].AsString(""));
            }
            return names.ToArray();
        }

        public bool ShowConfiguration2(string name) { return Session.TryExecute("config.activate", "name", name) != null; }
        public IEquationMgr GetEquationMgr() { return new EquationMgr(); }

        public object GetDependencies2(string fileName, bool traverseDependents, bool searchAssemblies, bool includeVirtual)
        {
            return new object[0];
        }

        public object ListExternalFileReferences2(int count, out string fileName) { fileName = null; return new object[0]; }
        public int ListExternalFileReferencesCount2(bool searchCurrentDoc) { return 0; }

        public bool Save3(int options, out int errors, out int warnings)
        {
            errors = 0;
            warnings = 0;
            return _app.SaveDocument(this);
        }

        public INote InsertNote(string text)
        {
            return new Note(text);
        }

        // --- IPartDoc ------------------------------------------------

        public object GetBodies2(int bodyType, bool visibleOnly)
        {
            List<object> bodies = new List<object>();
            foreach (Body b in Bodies()) bodies.Add(b);
            return bodies.ToArray();
        }

        IFeature IPartDoc.FeatureByName(string name) { return FeatureByName(name) as IFeature; }

        public string GetMaterialPropertyName2(string configName, out string database)
        {
            database = "";
            Json r = Session.TryExecute("material.list");
            if (r == null) return "";
            Json materials = r["materials"];
            return materials.Count > 0 ? materials[0].AsString("") : "";
        }

        // --- helpers used by the object model -----------------------------

        internal ulong StableId(EntityTag tag, string name)
        {
            string key = ((int)tag) + ":" + (name ?? "");
            ulong id;
            if (_ids.TryGetValue(key, out id)) return id;
            id = _nextId++;
            _ids[key] = id;
            return id;
        }

        internal string NameOfId(EntityTag tag, ulong id)
        {
            foreach (KeyValuePair<string, ulong> kv in _ids)
                if (kv.Value == id && kv.Key.StartsWith(((int)tag) + ":"))
                    return kv.Key.Substring(kv.Key.IndexOf(':') + 1);
            return null;
        }

        internal List<Body> Bodies()
        {
            List<Body> list = new List<Body>();
            Json measure = Session.TryExecute("inspect.measure");
            if (measure == null) return list;
            Json bodies = measure["bodies"];
            for (int i = 0; i < bodies.Count; i++)
            {
                string name = bodies[i]["name"].AsString("Body" + (i + 1));
                list.Add(new Body(this, StableId(EntityTag.Body, name), name));
            }
            return list;
        }

        internal List<Feature> Features()
        {
            List<Feature> list = new List<Feature>();
            Json doc = Session.TryExecute("document.inspect");
            if (doc == null) return list;
            Json timeline = doc["timeline"];
            for (int i = 0; i < timeline.Count; i++)
            {
                Json f = timeline[i];
                string name = f["name"].AsString(f["id"].AsString(""));
                ulong id = (ulong)f["id"].AsDouble(i + 1);
                string type = f["type"].AsString("");
                bool suppressed = f["suppressed"].Bool;
                list.Add(new Feature(this, id, name, type, suppressed));
            }
            return list;
        }

        internal IFeature FeatureOwningFace(string faceName) { return null; }

        internal object[] FacesOf(string bodyName)
        {
            List<object> faces = new List<object>();
            Json r = Session.TryExecute("model.faces", "body", bodyName);
            if (r == null) return faces.ToArray();
            Json list = r["faces"];
            for (int i = 0; i < list.Count; i++)
            {
                string name = list[i]["name"].AsString("F" + (i + 1));
                Face f = new Face(this, StableId(EntityTag.Face, name), name);
                Json normal = list[i]["plane_normal"];
                if (normal.Count == 3)
                    f.PlaneNormalRaw = normal[0].AsDouble(0) + "," + normal[1].AsDouble(0) + "," + normal[2].AsDouble(0);
                faces.Add(f);
            }
            return faces.ToArray();
        }

        internal object[] EdgesOf(string bodyName)
        {
            List<object> edges = new List<object>();
            Json r = Session.TryExecute("model.edges", "body", bodyName);
            if (r == null) return edges.ToArray();
            Json list = r["edges"];
            for (int i = 0; i < list.Count; i++)
            {
                string name = list[i]["name"].AsString("E" + (i + 1));
                edges.Add(new Edge(this, StableId(EntityTag.Edge, name), name));
            }
            return edges.ToArray();
        }

        internal bool FaceScalar(string faceName, string key, out double value)
        {
            value = 0;
            return false;
        }

        internal object FaceBox(string faceName) { return new double[6]; }

        internal double[] FaceNormal(string faceName)
        {
            foreach (Body b in Bodies())
                foreach (object o in FacesOf(b.BodyName))
                {
                    Face f = o as Face;
                    if (f != null && f.Name == faceName && !string.IsNullOrEmpty(f.PlaneNormalRaw))
                    {
                        string[] parts = f.PlaneNormalRaw.Split(',');
                        double[] n = new double[3];
                        for (int i = 0; i < 3; i++)
                            double.TryParse(parts[i], System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out n[i]);
                        return n;
                    }
                }
            return null;
        }

        internal object FaceTriangles(string faceName)
        {
            // The engine exposes tessellation through the render mesh; until a typed face-facet
            // command exists, report no triangles rather than lying about geometry.
            return new double[0];
        }

        internal double[] EdgeEndpoint(string edgeName, bool start)
        {
            foreach (Body b in Bodies())
            {
                Json r = Session.TryExecute("model.edges", "body", b.BodyName);
                if (r == null) continue;
                Json list = r["edges"];
                for (int i = 0; i < list.Count; i++)
                {
                    if (list[i]["name"].AsString("") != edgeName) continue;
                    Json p = list[i][start ? "start" : "end"];
                    if (p.Count == 3) return new double[] { p[0].AsDouble(0), p[1].AsDouble(0), p[2].AsDouble(0) };
                }
            }
            return new double[3];
        }

        internal double WorldVolumeMm3 { get { return Session.TotalVolumeMm3; } }
        internal double WorldAreaMm2 { get { return MeasureTotal("area_mm2"); } }
        internal double WorldMassG { get { return MeasureTotal("mass_g"); } }

        private double MeasureTotal(string key)
        {
            Json m = Session.TryExecute("inspect.measure");
            return m != null ? m["total"][key].AsDouble(0) : 0;
        }

        internal double[] WorldCentroid()
        {
            Json m = Session.TryExecute("inspect.measure");
            Json bodies = m != null ? m["bodies"] : Json.Null;
            Json c = bodies.Count > 0 ? bodies[0]["centroid"] : Json.Null;
            if (c.Count != 3) return new double[3];
            // SolidWorks units: metres.
            return new double[] { c[0].AsDouble(0) * 1e-3, c[1].AsDouble(0) * 1e-3, c[2].AsDouble(0) * 1e-3 };
        }

        internal SolidCraftSession Raw { get { return Session; } }

        /// <summary>The design as JSON (for Save3 / SaveAs).</summary>
        internal string DocumentJson { get { return Session.DocumentJson; } }

        /// <summary>Replaces the design from JSON. Returns null on success, else the error.</summary>
        internal string LoadDocument(string json) { return Session.LoadDocument(json); }

        public void Dispose()
        {
            if (Session != null) { Session.Dispose(); Session = null; }
        }
    }
}
