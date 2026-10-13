// The backing object model for the SolidCraft interop: the C# objects the SolidWorks interfaces
// return (features, bodies, faces, edges, sketches, dimensions), and the mapping between them and
// a SolidCraft session.
//
// The SolidWorks API is a live object graph: GetFaces() returns face objects you can hold, and a
// "persist reference" is an opaque byte[] that resolves back to one after a rebuild. SolidCraft's
// engine is command-based with stable *ids*, so this layer keeps a small registry: each entity the
// host has asked for gets a handle with a stable id, and the persist reference is a tagged encoding
// of that id, so it round-trips through GetPersistReference3 / GetObjectByPersistReference3.
using System;
using System.Collections.Generic;

namespace ModelForge.Interop.sldworks
{
    /// <summary>How an entity reference is tagged. The first byte of a persist reference.</summary>
    internal enum EntityTag : byte
    {
        Face = 1,
        Edge = 2,
        Vertex = 3,
        Feature = 4,
        Body = 5,
        Sketch = 6,
        Component = 7,
    }

    /// <summary>Encodes and decodes persist references (a stable id plus its kind).</summary>
    internal static class PersistReference
    {
        public static byte[] Encode(EntityTag tag, ulong id)
        {
            byte[] bytes = new byte[9];
            bytes[0] = (byte)tag;
            for (int i = 0; i < 8; i++) bytes[1 + i] = (byte)((id >> (8 * i)) & 0xFF);
            return bytes;
        }

        public static bool TryDecode(object reference, out EntityTag tag, out ulong id)
        {
            tag = 0;
            id = 0;
            byte[] bytes = reference as byte[];
            if (bytes == null || bytes.Length != 9) return false;
            tag = (EntityTag)bytes[0];
            ulong value = 0;
            for (int i = 0; i < 8; i++) value |= ((ulong)bytes[1 + i]) << (8 * i);
            id = value;
            if (tag < EntityTag.Face || tag > EntityTag.Component) return false;
            return true;
        }
    }

    /// <summary>Base for every object the interop hands out; carries its stable kind and id.</summary>
    internal abstract class EntityBase : IEntity
    {
        public EntityTag Tag { get; private set; }
        public ulong Id { get; private set; }
        public byte[] Reference { get { return PersistReference.Encode(Tag, Id); } }

        protected EntityBase(EntityTag tag, ulong id)
        {
            Tag = tag;
            Id = id;
        }

        public virtual IComponent2 GetComponent() { return null; }

        public bool Select4(bool append, SelectData data) { return Select4(append, data, false); }

        public virtual bool Select4(bool append, SelectData data, bool mark)
        {
            // Selection is tracked by the document; the default is a no-op success for entities the
            // document already knows how to address.
            return true;
        }
    }

    /// <summary>A body in the model.</summary>
    internal sealed class Body : EntityBase, IBody2
    {
        public string BodyName { get; private set; }
        private readonly Document _doc;

        public Body(Document doc, ulong id, string name) : base(EntityTag.Body, id)
        {
            _doc = doc;
            BodyName = name;
        }

        public string Name { get { return BodyName; } set { BodyName = value ?? BodyName; } }

        public object[] GetFaces()
        {
            return _doc.FacesOf(BodyName);
        }

        public object[] GetEdges()
        {
            return _doc.EdgesOf(BodyName);
        }

        public IMassProperty GetMassProperties()
        {
            return new MassProperty(_doc);
        }

        public ITessellation GetTessellation()
        {
            return new Tessellation(_doc, this);
        }
    }

    /// <summary>A face of a body.</summary>
    internal sealed class Face : EntityBase, IFace2
    {
        private readonly Document _doc;
        public string Name { get; private set; }
        public string PlaneNormalRaw { get; set; }

        public Face(Document doc, ulong id, string name) : base(EntityTag.Face, id)
        {
            _doc = doc;
            Name = name;
        }

        public double GetArea()
        {
            double area;
            return _doc.FaceScalar(Name, "area", out area) ? area : 0.0;
        }

        public object GetBox()
        {
            return _doc.FaceBox(Name);
        }

        public IFeature GetFeature()
        {
            return _doc.FeatureOwningFace(Name);
        }

        public ISurface GetSurface()
        {
            return new Surface(PlaneNormalRaw);
        }

        public object GetTessTriangles(bool noConversion)
        {
            return _doc.FaceTriangles(Name);
        }

        public double[] Normal
        {
            get
            {
                double[] n = _doc.FaceNormal(Name);
                return n ?? new double[3];
            }
        }
    }

    /// <summary>An edge of a body.</summary>
    internal sealed class Edge : EntityBase, IEdge
    {
        private readonly Document _doc;
        public string Name { get; private set; }

        public Edge(Document doc, ulong id, string name) : base(EntityTag.Edge, id)
        {
            _doc = doc;
            Name = name;
        }

        public ICurve GetCurve()
        {
            double[] start = _doc.EdgeEndpoint(Name, true);
            double[] end = _doc.EdgeEndpoint(Name, false);
            return new Curve(start, end);
        }

        public double[] GetCurveParams2()
        {
            return new double[] { 0, 0, 0, 0, 0, 0, 0, 1 };
        }

        public IVertex GetStartVertex() { return new Vertex(_doc.EdgeEndpoint(Name, true)); }

        public IVertex GetEndVertex() { return new Vertex(_doc.EdgeEndpoint(Name, false)); }

        public override bool Select4(bool append, SelectData data, bool mark)
        {
            // Record a world point on this edge so a fillet/chamfer can address it, matching the
            // SolidWorks behaviour where FeatureFillet acts on the current selection.
            if (!append) _doc.ClearSelectedEdges();
            double[] a = _doc.EdgeEndpoint(Name, true);
            double[] b = _doc.EdgeEndpoint(Name, false);
            double[] mid = new double[] { (a[0] + b[0]) / 2, (a[1] + b[1]) / 2, (a[2] + b[2]) / 2 };
            _doc.SelectEdgePoint(mid);
            return true;
        }
    }

    /// <summary>A straight-line curve between two points (SolidCraft edge geometry as a curve).</summary>
    internal sealed class Curve : ICurve
    {
        private readonly double[] _start;
        private readonly double[] _end;

        public Curve(double[] start, double[] end)
        {
            _start = start ?? new double[3];
            _end = end ?? new double[3];
        }

        public bool IsLine() { return true; }
        public bool IsCircle() { return false; }

        public double[] GetEndParams() { return new double[] { 0.0, 1.0 }; }

        public double[] Evaluate2(double parameter, int derivative)
        {
            if (derivative != 0) return new double[] { 0, 0, 0 };
            return new double[]
            {
                _start[0] + (_end[0] - _start[0]) * parameter,
                _start[1] + (_end[1] - _start[1]) * parameter,
                _start[2] + (_end[2] - _start[2]) * parameter,
            };
        }

        public object GetTessPts(double startTol, double endTol, object startPoint, object endPoint)
        {
            // A straight line tessellates to its two endpoints (6 doubles: start then end).
            return new double[] { _start[0], _start[1], _start[2], _end[0], _end[1], _end[2] };
        }

        public double[] CircleParams { get { return new double[0]; } }
        public double[] LineParams { get { return new double[] { _start[0], _start[1], _start[2], _end[0], _end[1], _end[2] }; } }
    }

    /// <summary>A plane (or other) surface behind a face.</summary>
    internal sealed class Surface : ISurface
    {
        private readonly string _normal;

        public Surface(string normal) { _normal = normal; }

        public bool IsPlane() { return !string.IsNullOrEmpty(_normal); }
        public bool IsCylinder() { return false; }
        public double[] PlaneParams { get { return ParseNormal(); } }
        public double[] CylinderParams { get { return new double[0]; } }

        private double[] ParseNormal()
        {
            if (string.IsNullOrEmpty(_normal)) return new double[6];
            string[] parts = _normal.Split(',');
            double[] result = new double[6];
            for (int i = 0; i < parts.Length && i < 6; i++)
            {
                double v;
                double.TryParse(parts[i], System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out v);
                result[i] = v;
            }
            return result;
        }
    }

    /// <summary>A vertex (an edge endpoint).</summary>
    internal sealed class Vertex : IVertex
    {
        private readonly double[] _point;
        public Vertex(double[] point) { _point = point ?? new double[3]; }
        public double[] GetPoint() { return _point; }
    }

    /// <summary>Mass properties of a body or the model, from the SolidCraft measure command.</summary>
    internal sealed class MassProperty : IMassProperty
    {
        private readonly Document _doc;
        public MassProperty(Document doc) { _doc = doc; }
        public double Volume { get { return _doc.WorldVolumeMm3 * 1e-9; } }   // SolidWorks works in m³
        public double SurfaceArea { get { return _doc.WorldAreaMm2 * 1e-6; } }
        public double Mass { get { return _doc.WorldMassG * 1e-3; } }
        public double[] CenterOfMass { get { return _doc.WorldCentroid(); } }
    }

    /// <summary>Tessellation of a body's faces. Faces already carry engine triangles.</summary>
    internal sealed class Tessellation : ITessellation
    {
        private readonly Document _doc;
        private readonly Body _body;
        public bool NeedFaceFacetMap { get; set; }
        public bool NeedVertexNormal { get; set; }

        public Tessellation(Document doc, Body body)
        {
            _doc = doc;
            _body = body;
        }

        public void Tessellate() { }

        public object[] GetFaceFacets(IFace2 face)
        {
            Face f = face as Face;
            if (f == null) return new object[0];
            List<object> facets = new List<object>();
            object triangles = _doc.FaceTriangles(f.Name);
            Array data = triangles as Array;
            if (data != null)
            {
                int count = data.Length / 9;
                for (int i = 0; i < count; i++) facets.Add(i);
            }
            return facets.ToArray();
        }

        public object[] GetFacetFins(int facet) { return new object[0]; }
        public object[] GetFinVertices(int fin) { return new object[0]; }
        public double[] GetVertexNormal(int vertex) { return new double[3]; }
        public double[] GetVertexPoint(int vertex) { return new double[3]; }
    }
}
