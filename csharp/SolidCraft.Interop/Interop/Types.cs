// Shared types for the SolidCraft C ABI — the C# mirror of `crates/ffi/src/types.rs`.
//
// The byte layout of every struct here must match the Rust definition exactly. The sizes are
// checked at startup by `Native.VerifyLayout()`, which calls `sldcraft_layout_guard` with these
// sizeof values; a mismatch stops the process with a clear message instead of corrupting memory.
//
// Keep this file and `crates/ffi/src/types.rs` in lockstep. `tools/gen-ffi.py` re-derives the
// `[StructLayout]` blocks from the Rust source, and `cargo xtask csharp` fails if they drift.
using System;
using System.Runtime.InteropServices;

namespace SolidCraft.Interop
{
    /// <summary>Status code returned by every fallible entry point. Mirrors Rust `ScStatus`.</summary>
    public enum ScStatus
    {
        Ok = 0,
        NullHandle = 1,
        UnknownCommand = 2,
        BadParams = 3,
        Engine = 4,
        Internal = 5,
        NotFinite = 6,
        BufferTooSmall = 7,
        BadUtf8 = 8,
    }

    /// <summary>What kind of model entity a reference points at. Mirrors Rust `ScEntityKind`.</summary>
    public enum ScEntityKind
    {
        None = 0,
        Face = 1,
        Edge = 2,
        Vertex = 3,
        Feature = 4,
        Body = 5,
        Sketch = 6,
        Component = 7,
    }

    /// <summary>A 3-vector. Mirrors Rust `ScVec3` (24 bytes).</summary>
    [StructLayout(LayoutKind.Sequential)]
    public struct ScVec3
    {
        public double X;
        public double Y;
        public double Z;

        public ScVec3(double x, double y, double z) { X = x; Y = y; Z = z; }

        /// <summary>True when every component is finite.</summary>
        public bool IsFinite { get { return !double.IsNaN(X) && !double.IsInfinity(X) && !double.IsNaN(Y) && !double.IsInfinity(Y) && !double.IsNaN(Z) && !double.IsInfinity(Z); } }

        public double[] ToArray() { return new[] { X, Y, Z }; }

        public override string ToString() { return "(" + X + ", " + Y + ", " + Z + ")"; }
    }

    /// <summary>An axis frame. Mirrors Rust `ScFrame` (72 bytes).</summary>
    [StructLayout(LayoutKind.Sequential)]
    public struct ScFrame
    {
        public ScVec3 Origin;
        public ScVec3 XDir;
        public ScVec3 YDir;
    }

    /// <summary>A stable, document-scoped entity reference. Mirrors Rust `ScEntityRef` (24 bytes).</summary>
    [StructLayout(LayoutKind.Sequential)]
    public struct ScEntityRef
    {
        public ScEntityKind Kind;
        public ulong Doc;
        public ulong Id;

        public static ScEntityRef None { get { return new ScEntityRef { Kind = ScEntityKind.None, Doc = 0, Id = 0 }; } }

        public bool IsNone { get { return Kind == ScEntityKind.None; } }

        public override string ToString() { return Kind + "#" + Id; }
    }

    /// <summary>
    /// A caller-owned string returned across the ABI. Mirrors Rust `ScOwnedStr` (16 bytes).
    /// Release with <see cref="Native.OwnedStrFree"/> when <see cref="Ptr"/> is non-null.
    /// </summary>
    [StructLayout(LayoutKind.Sequential)]
    public struct ScOwnedStr
    {
        public IntPtr Ptr;
        public UIntPtr Len;

        /// <summary>Reads the string and frees the buffer.</summary>
        public string Take()
        {
            if (Ptr == IntPtr.Zero) return null;
            try { return Marshal.PtrToStringUTF8(Ptr); }
            finally { Native.Invoke.OwnedStrFree(this); }
        }
    }

    /// <summary>Mass properties of a body or the whole model. Mirrors Rust `ScMassProperties` (128 bytes).</summary>
    [StructLayout(LayoutKind.Sequential)]
    public struct ScMassProperties
    {
        public double VolumeMm3;
        public double AreaMm2;
        public double MassG;
        public ScVec3 Centroid;
        public ScVec3 BBoxMin;
        public ScVec3 BBoxMax;
        public ulong Faces;
        public ulong Edges;
        public ulong Vertices;
        [MarshalAs(UnmanagedType.I1)] public bool Analytic;
    }
}
