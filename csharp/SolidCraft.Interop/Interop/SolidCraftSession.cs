// SolidCraftSession — the C# owner of one native SolidCraft session.
//
// This is the bridge the SolidWorks interface classes in this assembly are built on. It owns the
// native handle and offers two families of calls, matching the Rust ABI:
//
//   * typed: Measure(), BodyCount(), BodyFaces(), EdgeEndpoints(), SketchRectangle(), Extrude()…
//     Each returns or accepts the shared Sc* types, so a caller cannot pass the wrong shape.
//   * command: Execute(id, …) and ExecuteJson(…) — the engine's own command surface, the escape
//     hatch for anything without a typed wrapper yet.
//
// The constructor verifies the native library's ABI version and the shared-type layouts, so a
// version or layout drift fails loudly at startup.
using System;
using System.IO;
using System.Collections.Generic;
using System.Globalization;

namespace SolidCraft.Interop
{
    /// <summary>Raised when a SolidCraft command fails, naming the command that failed.</summary>
    public sealed class SolidCraftCommandException : Exception
    {
        public string Command { get; private set; }

        public SolidCraftCommandException(string command, string message) : base(message)
        {
            Command = command;
        }
    }

    /// <summary>One in-process SolidCraft session.</summary>
    public sealed class SolidCraftSession : IDisposable
    {
        private readonly IntPtr _handle;
        private bool _disposed;

        public SolidCraftSession()
        {
            if (!Native.Available)
                throw new SolidCraftNativeException(Native.LoadError ?? "the SolidCraft native library is unavailable");
            Native.VerifyLayout();
            _handle = Native.EntryPoints.sldcraft_session_new();
            if (_handle == IntPtr.Zero) throw new SolidCraftNativeException("SolidCraft refused to create a session");
        }

        internal IntPtr Handle { get { return _handle; } }

        // --- typed layer --------------------------------------------------

        /// <summary>Measures the whole model. Throws on failure.</summary>
        public ScMassProperties Measure()
        {
            RequireLive();
            ScMassProperties mp;
            ScStatus status = Native.EntryPoints.sldcraft_measure(_handle, out mp);
            ThrowIfError("inspect.measure", status);
            return mp;
        }

        /// <summary>Tries to measure the whole model. Returns false and an error message on failure.</summary>
        public bool TryMeasure(out ScMassProperties result, out string error)
        {
            RequireLive();
            ScStatus status = Native.EntryPoints.sldcraft_measure(_handle, out result);
            error = status == ScStatus.Ok ? null : Describe("inspect.measure", status);
            return status == ScStatus.Ok;
        }

        /// <summary>The total model volume in mm³ (NaN when it cannot be measured).</summary>
        public double TotalVolumeMm3 { get { RequireLive(); return Native.EntryPoints.sldcraft_total_volume_mm3(_handle); } }

        /// <summary>The number of bodies in the model.</summary>
        public ulong BodyCount()
        {
            RequireLive();
            ulong count;
            ThrowIfError("body count", Native.EntryPoints.sldcraft_body_count(_handle, out count));
            return count;
        }

        /// <summary>The faces of a body (empty name = the first body), in engine order.</summary>
        public ScEntityRef[] BodyFaces(string body) { return EntityList(Native.Invoke.BodyFaces, body, "model.faces"); }

        /// <summary>The edges of a body (empty name = the first body), in engine order.</summary>
        public ScEntityRef[] BodyEdges(string body) { return EntityList(Native.Invoke.BodyEdges, body, "model.edges"); }

        /// <summary>The world endpoints of a straight edge, or null when the edge is curved.</summary>
        public ScVec3[] EdgeEndpoints(ScEntityRef edge)
        {
            RequireLive();
            ScVec3[] points = new ScVec3[2];
            ScStatus status = Native.EntryPoints.sldcraft_edge_endpoints(_handle, edge, points);
            if (status == ScStatus.Ok) return points;
            if (status == ScStatus.Engine) return null;   // curved edges need tessellation
            ThrowIfError("edge endpoints", status);
            return null;
        }

        /// <summary>Reads a numeric design parameter (a driving dimension) by name.</summary>
        public bool TryParameter(string name, out double value)
        {
            RequireLive();
            return Native.Invoke.Parameter(_handle, name, out value) == ScStatus.Ok;
        }

        /// <summary>Creates a corner rectangle on the active sketch; returns its four edges.</summary>
        public ScEntityRef[] SketchRectangle(double x0, double y0, double x1, double y1)
        {
            RequireLive();
            ScEntityRef[] edges = new ScEntityRef[4];
            UIntPtr written;
            ScStatus status = Native.EntryPoints.sldcraft_sketch_rectangle(_handle, new ScVec3(x0, y0, 0), new ScVec3(x1, y1, 0), edges, (UIntPtr)edges.Length, out written);
            ThrowIfError("sketch.rectangle.two_point", status);
            int n = (int)written;
            if (n == edges.Length) return edges;
            ScEntityRef[] exact = new ScEntityRef[n];
            Array.Copy(edges, exact, n);
            return exact;
        }

        /// <summary>Sets a driving dimension on a sketch curve, then rebuilds.</summary>
        public void SketchDimension(ScEntityRef entity, double value)
        {
            RequireLive();
            ThrowIfError("sketch.dimension", Native.EntryPoints.sldcraft_sketch_dimension(_handle, entity, value));
        }

        /// <summary>Extrudes the active sketch profile; returns the new feature id.</summary>
        public ulong Extrude(double distanceMm, bool cut)
        {
            RequireLive();
            ulong feature;
            ThrowIfError("solid.extrude", Native.EntryPoints.sldcraft_extrude(_handle, distanceMm, cut, out feature));
            return feature;
        }

        // --- command layer (escape hatch) ---------------------------------

        /// <summary>Runs a command and returns its JSON result, or throws.</summary>
        public Json Execute(string id, params object[] keyValues)
        {
            return ExecuteRaw(Json.Command(id, keyValues), id);
        }

        /// <summary>Runs a command whose params object has already been built as JSON.</summary>
        public Json ExecuteJson(string commandJson, string idForErrors)
        {
            return ExecuteRaw(commandJson, idForErrors);
        }

        private Json ExecuteRaw(string commandJson, string idForErrors)
        {
            RequireLive();
            string reply = Native.Invoke.Execute(_handle, commandJson);
            Json parsed = Json.Parse(reply);
            if (parsed == null) throw new SolidCraftCommandException(idForErrors, "the SolidCraft library returned no reply");
            if (!parsed["ok"].Bool)
                throw new SolidCraftCommandException(idForErrors, parsed["error"].AsString("unknown error"));
            return parsed["result"];
        }

        /// <summary>Runs a command and returns the result, or null on failure (no exception).</summary>
        public Json TryExecute(string id, params object[] keyValues)
        {
            try { return Execute(id, keyValues); }
            catch (SolidCraftCommandException) { return null; }
            catch (SolidCraftNativeException) { return null; }
        }

        // --- documents ----------------------------------------------------

        /// <summary>Serialises the design document as JSON.</summary>
        public string DocumentJson { get { RequireLive(); return Native.Invoke.DocumentJson(_handle); } }

        /// <summary>Replaces the design document from JSON. Returns null on success, else the error.</summary>
        public string LoadDocument(string json)
        {
            RequireLive();
            ScStatus status = Native.Invoke.LoadDocument(_handle, json);
            return status == ScStatus.Ok ? null : status.ToString();
        }

        // --- internals ----------------------------------------------------

        private delegate ScStatus EntityListCall(IntPtr handle, string body, ScEntityRef[] outRefs, UIntPtr cap, out UIntPtr written);

        private ScEntityRef[] EntityList(EntityListCall call, string body, string what)
        {
            RequireLive();
            // First ask for the count with a small buffer, then grow.
            ScEntityRef[] buffer = new ScEntityRef[16];
            UIntPtr written;
            ScStatus status = call(_handle, body, buffer, (UIntPtr)buffer.Length, out written);
            int n = (int)written;
            if (status == ScStatus.BufferTooSmall)
            {
                buffer = new ScEntityRef[Math.Max(n, 1)];
                status = call(_handle, body, buffer, (UIntPtr)buffer.Length, out written);
                n = (int)written;
            }
            ThrowIfError(what, status);
            if (n == buffer.Length) return buffer;
            ScEntityRef[] exact = new ScEntityRef[n];
            Array.Copy(buffer, exact, n);
            return exact;
        }

        private void RequireLive()
        {
            if (_disposed) throw new ObjectDisposedException("SolidCraftSession");
        }

        private void ThrowIfError(string what, ScStatus status)
        {
            if (status != ScStatus.Ok) throw new SolidCraftCommandException(what, Describe(what, status));
        }

        private static string Describe(string what, ScStatus status)
        {
            switch (status)
            {
                case ScStatus.NullHandle: return what + ": null session handle";
                case ScStatus.UnknownCommand: return what + ": unknown command";
                case ScStatus.BadParams: return what + ": invalid parameters";
                case ScStatus.Engine: return what + ": engine error";
                case ScStatus.Internal: return what + ": internal error (the design was kept as it was)";
                case ScStatus.NotFinite: return what + ": a value was not finite";
                case ScStatus.BufferTooSmall: return what + ": buffer too small";
                case ScStatus.BadUtf8: return what + ": a string was not valid UTF-8";
                default: return what + ": " + status;
            }
        }

        public void Dispose()
        {
            if (_disposed) return;
            _disposed = true;
            Native.EntryPoints.sldcraft_session_free(_handle);
        }
    }
}
