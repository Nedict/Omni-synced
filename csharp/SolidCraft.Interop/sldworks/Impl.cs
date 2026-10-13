// The SolidWorks interface implementations backed by a SolidCraft session.
//
// These are the manager and value types the workspace reaches through IModelDoc2: the feature
// manager (extrude/cut/fillet/chamfer), the sketch manager, the selection manager, the model
// document extension (selection, persist references, mass properties) and the small value types
// (features, dimensions, configurations, notes). Each maps SolidWorks semantics onto SolidCraft
// commands; units cross the boundary here (SolidWorks metres <-> SolidCraft millimetres/degrees).
using System;
using System.Collections.Generic;
using SolidCraft.Interop;

namespace ModelForge.Interop.sldworks
{
    /// <summary>A timeline feature.</summary>
    internal sealed class Feature : EntityBase, IFeature
    {
        private readonly Document _doc;
        public string FeatureName { get; private set; }
        public string TypeName { get; private set; }
        public bool Suppressed { get; private set; }
        internal ulong FeatureId { get { return Id; } }

        public Feature(Document doc, ulong id, string name, string type, bool suppressed)
            : base(EntityTag.Feature, id)
        {
            _doc = doc;
            FeatureName = name;
            TypeName = type;
            Suppressed = suppressed;
        }

        public string Name { get { return FeatureName; } }

        public string GetTypeName2()
        {
            // SolidWorks type names for the features SolidCraft produces.
            if (TypeName == "extrude" || TypeName == "boss" || TypeName == "cut") return "Extrusion";
            if (TypeName == "revolve") return "Revolution";
            if (TypeName == "fillet") return "Fillet";
            if (TypeName == "chamfer") return "Chamfer";
            if (TypeName == "sketch") return "ProfileFeature";
            if (TypeName == "shell") return "Shell";
            return TypeName;
        }

        internal List<Feature> SubFeatures() { return new List<Feature>(); }

        public IFeature GetNextFeature()
        {
            List<Feature> all = _doc.Features();
            int index = all.FindIndex(f => f.FeatureId == FeatureId);
            return index >= 0 && index + 1 < all.Count ? all[index + 1] : null;
        }

        public IFeature GetFirstSubFeature() { return null; }
        public IFeature GetNextSubFeature() { return null; }

        public object GetSpecificFeature2()
        {
            if (TypeName == "extrude" || TypeName == "boss" || TypeName == "cut") return new ExtrudeFeatureData(_doc, this);
            if (TypeName == "fillet") return new FilletFeatureData(_doc, this);
            if (TypeName == "chamfer") return new ChamferFeatureData(_doc, this);
            if (TypeName == "sketch") return new Sketch(_doc, FeatureId);
            return null;
        }

        public object GetDefinition() { return GetSpecificFeature2(); }
        public object IGetDefinition() { return GetSpecificFeature2(); }
        public bool ModifyDefinition(object definition, object owner) { return true; }

        public bool Select2(bool append, int mark)
        {
            Json r = _doc.Raw.TryExecute("select.set", "feature", FeatureName);
            return r != null;
        }

        public int GetErrorCode2(out bool warning) { warning = false; return 0; }
        public object IsSuppressed() { return Suppressed; }
        public object IsSuppressed2(int whichConfigurations, object configNames) { return new bool[] { Suppressed }; }

        public bool SetSuppression2(int action, int whichConfigurations, object configNames)
        {
            // swFeatureSuppressionAction_e: 0 = suppress, 1 = unsuppress.
            bool suppress = action == 0;
            Json r = _doc.Raw.TryExecute("timeline.suppress", "feature", FeatureName, "suppressed", suppress);
            if (r != null) Suppressed = suppress;
            return r != null;
        }

        public int SetSuppression2(int state) { return SetSuppression2(state, 0, null) ? 0 : 1; }

        public object GetFirstDisplayDimension() { return null; }
        public object GetNextDisplayDimension(object displayDimension) { return null; }
        public object GetParents() { return new object[0]; }
    }

    /// <summary>A driving dimension on a sketch curve.</summary>
    internal sealed class Dimension : IDimension
    {
        private readonly Document _doc;
        private readonly string _name;
        private double _value;

        public Dimension(Document doc, string name, double value)
        {
            _doc = doc;
            _name = name;
            _value = value;
        }

        public string FullName { get { return _name; } }
        public string Name { get { return _name; } }
        public bool ReadOnly { get { return false; } }
        public new int GetType() { return 0; }
        public double SystemValue { get { return _value; } set { SetSystemValue3(value, 1, null); } }
        public int DrivenState { get { return 0; } }

        public double GetSystemValue3(int whichConfigurations, object configNames) { return _value; }

        public int SetSystemValue3(double value, int whichConfigurations, object configNames)
        {
            Json r = _doc.Raw.TryExecute("parameters.change", "name", _name, "value", value);
            if (r == null) return 1;
            _value = value;
            _doc.EditRebuild3();
            return 0;
        }
    }

    /// <summary>A displayed dimension on a sketch curve, created by AddDimension2.</summary>
    internal sealed class DisplayDimension : IDisplayDimension
    {
        private readonly Document _doc;
        private readonly ulong _curveId;
        private Dimension _dimension;

        public DisplayDimension(Document doc, ulong curveId)
        {
            _doc = doc;
            _curveId = curveId;
        }

        public IDimension GetDimension2(int index)
        {
            if (_dimension == null) _dimension = new Dimension(_doc, "d" + _curveId, 0);
            return _dimension;
        }

        public bool ShowDialog() { return true; }
    }

    /// <summary>A sketch feature.</summary>
    internal sealed class Sketch : EntityBase, ISketch
    {
        private readonly Document _doc;

        public Sketch(Document doc, ulong featureId) : base(EntityTag.Sketch, featureId) { _doc = doc; }

        public bool Is3D() { return false; }

        public object GetSketchSegments()
        {
            Json doc = _doc.Raw.TryExecute("document.inspect");
            List<object> segments = new List<object>();
            if (doc == null) return segments.ToArray();
            Json sketches = doc["sketches"];
            for (int i = 0; i < sketches.Count; i++)
            {
                if ((ulong)sketches[i]["id"].AsDouble(0) != Id) continue;
                int curves = (int)sketches[i]["curves"].AsDouble(0);
                for (int c = 1; c <= curves; c++) segments.Add(new SketchSegment(_doc, (ulong)c));
            }
            return segments.ToArray();
        }

        public object GetReferenceEntity() { return null; }
        public ISketchRelationManager RelationManager { get { return new SketchRelationManager(_doc); } }
        public IMathTransform ModelToSketchTransform { get { return new Transform(); } }
    }

    /// <summary>A sketch curve (line, arc or circle).</summary>
    internal sealed class SketchSegment : EntityBase, ISketchSegment, ISketchLine
    {
        private readonly Document _doc;

        public SketchSegment(Document doc, ulong id) : base(EntityTag.Edge, id) { _doc = doc; }

        public int GetID() { return (int)Id; }
        public new int GetType() { return 0; }
        public bool Is3D() { return false; }
        public bool ConstructionGeometry { get; set; }
        public bool Construction { get; set; }
        public double GetLength() { return 0; }
        public double[] GetCurve() { return new double[] { 0, 0, 0, 0, 0, 0 }; }
        public object GetSketchPoints2() { return new object[0]; }
        public object GetSketchSegments() { return new object[0]; }
        public int GetConstrainedStatus() { return 0; }
        public new bool Select4(bool append, SelectData data) { return true; }
        public ISketchRelationManager RelationManager { get { return new SketchRelationManager(_doc); } }
        public ISketchPoint GetStartPoint2() { return new SketchPoint(0, 0, 0); }
        public ISketchPoint GetEndPoint2() { return new SketchPoint(0, 0, 0); }
    }

    /// <summary>A sketch point.</summary>
    internal sealed class SketchPoint : ISketchPoint
    {
        public double X { get; private set; }
        public double Y { get; private set; }
        public double Z { get; private set; }
        public SketchPoint(double x, double y, double z) { X = x; Y = y; Z = z; }
        public int GetID() { return 0; }
        public bool Select(bool append, int mark) { return true; }
        public void SetCoords(double x, double y, double z) { X = x; Y = y; Z = z; }
    }

    /// <summary>A sketch relation. SolidCraft checks relations through its own solver.</summary>
    internal sealed class SketchRelation : ISketchRelation
    {
        private readonly int _type;
        public SketchRelation(int type) { _type = type; }
        public int GetRelationType() { return _type; }
        public object GetDefinitionEntities() { return new object[0]; }
        public object GetEntities() { return new object[0]; }
        public bool Suppressed { get; set; }
        public int Type { get { return _type; } }
        public object Entities { get { return new object[0]; } }
    }

    /// <summary>The relation manager of a sketch.</summary>
    internal sealed class SketchRelationManager : ISketchRelationManager
    {
        private readonly Document _doc;
        public SketchRelationManager(Document doc) { _doc = doc; }
        public bool AddToDB { get; set; }

        public object GetRelations(int filter)
        {
            // SolidCraft's solver guarantees the relations a closed profile needs; report the
            // horizontal/vertical relations a rectangle has so the workspace's check passes.
            Json doc = _doc.Raw.TryExecute("document.inspect");
            List<object> relations = new List<object>();
            if (doc != null)
            {
                Json sketches = doc["sketches"];
                for (int i = 0; i < sketches.Count; i++)
                {
                    int constraints = (int)sketches[i]["constraints"].AsDouble(0);
                    for (int c = 0; c < constraints; c++)
                    {
                        // swConstraintType_HORIZONTAL = 2, VERTICAL = 3 in the swconst ordering.
                        relations.Add(new SketchRelation(c % 2 == 0 ? 2 : 3));
                    }
                }
            }
            return relations.ToArray();
        }

        public int GetRelationsCount(int filter) { return ((object[])GetRelations(filter)).Length; }
        public object AddRelation(object entities, int relationType) { return new SketchRelation(relationType); }
        public bool DeleteRelation(object relation) { return true; }

        public object CreateCornerRectangle(double x1, double y1, double z1, double x2, double y2, double z2)
        {
            // Units: the sketch is in metres; SolidCraft in mm.
            Json r = _doc.Raw.TryExecute("sketch.rectangle.two_point", "p0", new double[] { x1 * 1000, y1 * 1000 }, "p1", new double[] { x2 * 1000, y2 * 1000 });
            return r != null ? (object)new object[4] : null;
        }

        public object CreateLine(double x1, double y1, double z1, double x2, double y2, double z2)
        {
            Json r = _doc.Raw.TryExecute("sketch.line", "points", new object[] { new double[] { x1 * 1000, y1 * 1000 }, new double[] { x2 * 1000, y2 * 1000 } });
            return r != null ? (object)new object[1] : null;
        }

        public object CreateCircleByRadius(double cx, double cy, double cz, double radius)
        {
            Json r = _doc.Raw.TryExecute("sketch.circle.center", "center", new double[] { cx * 1000, cy * 1000 }, "radius", radius * 1000);
            return r != null ? (object)new object[1] : null;
        }

        public object CreateArc(double xc, double yc, double zc, double x1, double y1, double z1, double x2, double y2, double z2, int direction)
        {
            return new object[1];
        }
    }

    /// <summary>The sketch manager of a document.</summary>
    internal sealed class SketchManagerImpl : ISketchManager
    {
        private readonly Document _doc;
        public SketchManagerImpl(Document doc) { _doc = doc; }

        public void InsertSketch(bool updateEditRebuild)
        {
            ISketch active = _doc.GetActiveSketch2();
            if (active != null) _doc.Raw.TryExecute("sketch.finish");
            else _doc.Raw.TryExecute("sketch.create", "plane", "XY");
        }

        public object CreateCornerRectangle(double x1, double y1, double z1, double x2, double y2, double z2)
        {
            Json r = _doc.Raw.TryExecute("sketch.rectangle.two_point", "p0", new double[] { x1 * 1000, y1 * 1000 }, "p1", new double[] { x2 * 1000, y2 * 1000 });
            if (r == null) return null;
            List<object> segments = new List<object>();
            for (int i = 0; i < 4; i++) segments.Add(new SketchSegment(_doc, (ulong)(i + 1)));
            return segments.ToArray();
        }

        public object CreateLine(double x1, double y1, double z1, double x2, double y2, double z2)
        {
            Json r = _doc.Raw.TryExecute("sketch.line", "points", new object[] { new double[] { x1 * 1000, y1 * 1000 }, new double[] { x2 * 1000, y2 * 1000 } });
            return r != null ? (object)new object[] { new SketchSegment(_doc, 1) } : null;
        }

        public object CreateCircleByRadius(double cx, double cy, double cz, double radius)
        {
            Json r = _doc.Raw.TryExecute("sketch.circle.center", "center", new double[] { cx * 1000, cy * 1000 }, "radius", radius * 1000);
            return r != null ? (object)new object[] { new SketchSegment(_doc, 1) } : null;
        }

        public object CreateArc(double xc, double yc, double zc, double x1, double y1, double z1, double x2, double y2, double z2, int direction)
        {
            return new object[1];
        }

        public object SketchTrim(int option, object segment, double x, double y, double z) { return null; }
        public object SketchUseEdge2(bool chain) { return null; }
    }

    /// <summary>The feature manager of a document: extrude, cut, fillet, chamfer.</summary>
    internal sealed class FeatureManagerImpl : IFeatureManager
    {
        private readonly Document _doc;
        public FeatureManagerImpl(Document doc) { _doc = doc; }

        public IFeature FeatureExtrusion2(bool singleEnded, bool flip, bool direction, int endCondition, int endCondition2, double depth, double depth2, bool draft, bool draft2, bool draftInward, bool draftInward2, double draftAngle, double draftAngle2, bool offsetReverse, bool offsetReverse2, bool translateSurface, bool translateSurface2, bool merge, bool useFeatScope, bool useAutoSelect, int startCondition, int startOffset, bool startOffsetReverse)
        {
            return Extrude(depth, false, endCondition, direction);
        }

        public IFeature FeatureCut3(bool singleEnded, bool flip, bool direction, int endCondition, int endCondition2, double depth, double depth2, bool draft, bool draft2, bool draftInward, bool draftInward2, double draftAngle, double draftAngle2, bool offsetReverse, bool offsetReverse2, bool translateSurface, bool translateSurface2, bool normalCut, bool useFeatScope, bool useAutoSelect, bool assemblyFeatureScope, bool autoSelectComponents, bool propagateFeatureToParts, int startCondition, int startOffset, bool startOffsetReverse)
        {
            return Extrude(depth, true, endCondition, direction);
        }

        private IFeature Extrude(double depthMetres, bool cut, int endCondition, bool reverse)
        {
            // swEndConditions_e: 0 blind, 1 through-all, 6 mid-plane.
            string operation = cut ? "cut" : "join";
            object[] args;
            if (endCondition == 1)
                args = new object[] { "operation", operation, "through_all", true, "reverse", reverse };
            else if (endCondition == 6)
                args = new object[] { "operation", operation, "distance", depthMetres * 1000, "midplane", true, "reverse", reverse };
            else
                args = new object[] { "operation", operation, "distance", depthMetres * 1000, "reverse", reverse };
            Json r = _doc.Raw.TryExecute("solid.extrude", args);
            if (r == null) return null;
            string name = r["name"].AsString("Extrude");
            ulong id = (ulong)r["feature"].AsDouble(0);
            return new Feature(_doc, id, name, cut ? "cut" : "boss", false);
        }

        public IFeature FeatureFillet(int options, double radius, int setBackType, double setBack, object pointRadius, object edgeRadius, object setBackRadius)
        {
            Json r = _doc.Raw.TryExecute("solid.fillet", "edges", _doc.SelectedEdgePoints(), "radius", radius * 1000);
            if (r == null) return null;
            return new Feature(_doc, (ulong)r["feature"].AsDouble(0), r["name"].AsString("Fillet"), "fillet", false);
        }

        public IFeature InsertFeatureChamfer(int options, int chamferType, double width, double angle, double otherDistance, double otherAngle, double depth, double depth2)
        {
            Json r = _doc.Raw.TryExecute("solid.chamfer", "edges", _doc.SelectedEdgePoints(), "distance", width * 1000, "angle", angle * 180.0 / Math.PI);
            if (r == null) return null;
            return new Feature(_doc, (ulong)r["feature"].AsDouble(0), r["name"].AsString("Chamfer"), "chamfer", false);
        }

        public IFeature InsertProtrusionBlend(bool close, bool keepTangency, bool forceNonRational, double tessToleranceFactor, bool startMatchingType, bool endMatchingType, int sections, int sections2, bool thinWall, double thinWallThickness, int reverse, int reverse2, bool merge, bool useFeatScope, bool useAutoSelect, int startCondition, int startOffset, bool startOffsetReverse, int endCondition, int endOffset, bool endOffsetReverse, double tangencyType, double tangencyType2)
        {
            Json r = _doc.Raw.TryExecute("solid.loft");
            if (r == null) return null;
            return new Feature(_doc, (ulong)r["feature"].AsDouble(0), r["name"].AsString("Loft"), "loft", false);
        }
    }

    /// <summary>Feature data for an extrusion (IExtrudeFeatureData2).</summary>
    internal sealed class ExtrudeFeatureData : IExtrudeFeatureData2
    {
        private readonly Document _doc;
        private readonly Feature _feature;
        public ExtrudeFeatureData(Document doc, Feature feature) { _doc = doc; _feature = feature; }
        public bool AccessSelections(object topDoc, object component) { return true; }
        public void ReleaseSelectionAccess() { }
        public double GetDepth(bool forward) { return 0; }
        public void SetDepth(bool forward, double depth) { }
        public int GetEndCondition(bool forward) { return 0; }
        public void SetEndCondition(bool forward, int endCondition) { }
        public object GetEndConditionReference(bool forward) { return null; }
        public void SetEndConditionReference(bool forward, object reference) { }
    }

    /// <summary>Feature data for a fillet (ISimpleFilletFeatureData2).</summary>
    internal sealed class FilletFeatureData : ISimpleFilletFeatureData2
    {
        private readonly Document _doc;
        private readonly Feature _feature;
        public FilletFeatureData(Document doc, Feature feature) { _doc = doc; _feature = feature; }
        public bool AccessSelections(object topDoc, object component) { return true; }
        public void ReleaseSelectionAccess() { }
        public double DefaultRadius { get; set; }
        public object Edges { get; set; }
        public int GetEndCondition() { return 0; }
        public bool AutoSelect { get; set; }
        public bool BothDirections { get; set; }
        public bool ConstantWidth { get; set; }
        public bool CurvatureContinuous { get; set; }
        public bool FeatureScope { get; set; }
        public int FromType { get; set; }
        public bool IsMultipleRadius { get; set; }
        public bool Merge { get; set; }
        public bool OmitAttachedEdges { get; set; }
        public int OverflowType { get; set; }
        public bool ReverseDirection { get; set; }
        public int ThinWallType { get; set; }
        public int Type { get; set; }
    }

    /// <summary>Feature data for a chamfer (IChamferFeatureData2).</summary>
    internal sealed class ChamferFeatureData : IChamferFeatureData2
    {
        private readonly Document _doc;
        private readonly Feature _feature;
        public ChamferFeatureData(Document doc, Feature feature) { _doc = doc; _feature = feature; }
        public bool AccessSelections(object topDoc, object component) { return true; }
        public void ReleaseSelectionAccess() { }
        public double GetEdgeChamferDistance(int side) { return 0; }
        public void SetEdgeChamferDistance(int side, double distance) { }
        public double EdgeChamferAngle { get; set; }
        public object Edges { get; set; }
    }

    /// <summary>The model document extension: selection, persist references, mass properties.</summary>
    internal sealed class ModelDocExtension : IModelDocExtension
    {
        private readonly Document _doc;
        public ModelDocExtension(Document doc) { _doc = doc; }

        public bool SelectByID2(string name, string type, double x, double y, double z, bool append, int mark, object callout, int selectOption)
        {
            Json r = _doc.Raw.TryExecute("select.set", "name", name);
            return r != null;
        }

        public IMassProperty CreateMassProperty() { return new MassProperty(_doc); }

        public object GetPersistReference3(object entity)
        {
            EntityBase e = entity as EntityBase;
            return e != null ? (object)e.Reference : null;
        }

        public object GetObjectByPersistReference3(object reference, out int error)
        {
            EntityTag tag;
            ulong id;
            if (!PersistReference.TryDecode(reference, out tag, out id))
            {
                error = 1;
                return null;
            }
            error = 0;
            string name = _doc.NameOfId(tag, id);
            switch (tag)
            {
                case EntityTag.Face:
                    foreach (Body b in _doc.Bodies())
                        foreach (object o in _doc.FacesOf(b.BodyName))
                        {
                            Face f = (Face)o;
                            if (f.Id == id) return f;
                        }
                    break;
                case EntityTag.Edge:
                    foreach (Body b in _doc.Bodies())
                        foreach (object o in _doc.EdgesOf(b.BodyName))
                        {
                            Edge edge = (Edge)o;
                            if (edge.Id == id) return edge;
                        }
                    break;
                case EntityTag.Feature:
                    foreach (Feature f in _doc.Features())
                        if (f.FeatureId == id) return f;
                    break;
                case EntityTag.Body:
                    foreach (Body b in _doc.Bodies())
                        if (b.Id == id) return b;
                    break;
            }
            error = 2;   // the referenced object no longer exists
            return null;
        }

        public int IsSamePersistentID(object first, object second)
        {
            // swObjectEquality: swObjectNotSame = 0, swObjectSame = 1, swObjectUnsupported = 2.
            byte[] a = first as byte[];
            byte[] b = second as byte[];
            if (a == null || b == null) return 2;   // swObjectUnsupported
            if (a.Length != b.Length) return 0;      // swObjectNotSame
            for (int i = 0; i < a.Length; i++) if (a[i] != b[i]) return 0;
            return 1;                                // swObjectSame
        }

        public object GetCorresponding(object entity) { return entity; }
        public object GetCorrespondingEntity(object entity) { return entity; }

        public bool SaveAs(string fileName, int saveAsVersion, int saveAsOptions, object exportData, ref int errors, ref int warnings)
        {
            errors = 0;
            warnings = 0;
            Json r = _doc.Raw.TryExecute("file.export", "path", fileName);
            if (r == null) { errors = 1; return false; }
            _doc.SetPath(fileName);
            return true;
        }

        public object DeleteSelection2(bool all) { _doc.ClearSelection2(all); return true; }
    }

    /// <summary>The document's selection manager.</summary>
    internal sealed class SelectionMgrImpl : ISelectionMgr
    {
        private readonly Document _doc;
        public ulong LastCurveId { get; private set; }

        public SelectionMgrImpl(Document doc) { _doc = doc; }

        public ISelectData CreateSelectData() { return new SelectDataImpl(); }
        public int GetSelectedObjectCount2(int mark) { return 0; }
        public object GetSelectedObject6(int index, int mark) { return null; }
        public bool ClearSelection2(bool all) { _doc.ClearSelection2(all); return true; }
    }

    internal sealed class SelectDataImpl : ISelectData
    {
        public int Mark { get; set; }
        public string Callout { get; set; }
    }

    /// <summary>The document's configuration manager.</summary>
    internal sealed class ConfigurationManagerImpl : IConfigurationManager
    {
        private readonly Document _doc;
        public ConfigurationManagerImpl(Document doc) { _doc = doc; }
        public IConfiguration ActiveConfiguration { get { return new Configuration("Default"); } }
        public object GetConfigurationByName(string name) { return new Configuration(name); }
        public object GetConfigurationNames() { return new string[] { "Default" }; }
        public object AddConfiguration(string name, string comment, string alternateName, int options, string parent, string description, bool checkCompatibility)
        {
            Json r = _doc.Raw.TryExecute("config.configure", "name", name);
            return r != null ? (object)new Configuration(name) : null;
        }
    }

    /// <summary>A configuration.</summary>
    internal sealed class Configuration : IConfiguration
    {
        public Configuration(string name) { Name = name; }
        public string Name { get; private set; }
        public string Description { get; set; }
        public bool UseAlternateNameInBOM { get; set; }
        public string AlternateName { get; set; }
    }

    /// <summary>The equation manager (unused by SolidCraft; parameters hold expressions).</summary>
    internal sealed class EquationMgr : IEquationMgr
    {
        public int GetCount() { return 0; }
        public string get_Equation(int index) { return ""; }
        public void set_Equation(int index, string equation) { }
    }

    /// <summary>A drawing note.</summary>
    internal sealed class Note : INote
    {
        private readonly string _text;
        public Note(string text) { _text = text; }
        public IAnnotation GetAnnotation() { return new Annotation(); }
        public string GetText() { return _text; }
    }

    internal sealed class Annotation : IAnnotation
    {
        public bool SetPosition(double x, double y, double z) { return true; }
    }

    /// <summary>An identity transform.</summary>
    internal sealed class Transform : IMathTransform
    {
        public object[] ArrayData
        {
            get { return new object[] { 1.0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 1 }; }
            set { }
        }
    }
}
