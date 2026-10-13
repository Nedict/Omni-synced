// SolidCraft.Interop.Native — the P/Invoke layer over the solidcraft-ffi C ABI.
//
// This is the only place that talks to the Rust library. Every declaration here mirrors one
// `#[unsafe(no_mangle)] extern "C"` function in `crates/ffi/src/lib.rs`, and the marshalled types
// mirror `crates/ffi/src/types.rs` byte for byte (see Types.cs).
//
// The native library is `libsolidcraft_ffi.<ext>` (`.dylib` on macOS, `.so` on Linux,
// `solidcraft_ffi.dll` on Windows). It is located, in order, from:
//   1. the `SOLIDCRAFT_FFI_LIB` environment variable (a full path);
//   2. `libsolidcraft_ffi.*` next to this assembly;
//   3. the system search path.
using System;
using System.IO;
using System.Runtime.InteropServices;

namespace SolidCraft.Interop
{
    /// <summary>Raised when the native library cannot be loaded or its layout does not match.</summary>
    public sealed class SolidCraftNativeException : Exception
    {
        public SolidCraftNativeException(string message) : base(message) { }
    }

    public static class Native
    {
        internal const int AbiVersion = 2;
        private const string Library = "solidcraft_ffi";

        private static readonly object Gate = new object();
        private static bool _tried;
        private static string _resolvedPath;
        private static string _loadError;

        static Native()
        {
            NativeLibrary.SetDllImportResolver(typeof(Native).Assembly, Resolve);
        }

        private static IntPtr Resolve(string name, System.Reflection.Assembly assembly, DllImportSearchPath? search)
        {
            if (!string.Equals(name, Library, StringComparison.OrdinalIgnoreCase)) return IntPtr.Zero;
            foreach (string candidate in Candidates())
            {
                if (NativeLibrary.TryLoad(candidate, out IntPtr handle))
                {
                    _resolvedPath = candidate;
                    return handle;
                }
            }
            return IntPtr.Zero;
        }

        private static string FileName()
        {
            if (RuntimeInformation.IsOSPlatform(OSPlatform.OSX)) return "libsolidcraft_ffi.dylib";
            if (RuntimeInformation.IsOSPlatform(OSPlatform.Windows)) return "solidcraft_ffi.dll";
            return "libsolidcraft_ffi.so";
        }

        private static string[] Candidates()
        {
            string fromEnv = Environment.GetEnvironmentVariable("SOLIDCRAFT_FFI_LIB");
            if (!string.IsNullOrEmpty(fromEnv)) return new[] { fromEnv };
            string file = FileName();
            return new[] { Path.Combine(AppContext.BaseDirectory, file), file };
        }

        /// <summary>The path the native library was loaded from, or null.</summary>
        public static string ResolvedPath { get { EnsureLoaded(); return _resolvedPath; } }

        /// <summary>The load error, or null when the library is usable.</summary>
        public static string LoadError { get { EnsureLoaded(); return _loadError; } }

        public static bool Available { get { EnsureLoaded(); return _loadError == null; } }

        private static void EnsureLoaded()
        {
            lock (Gate)
            {
                if (_tried) return;
                _tried = true;
                try
                {
                    int abi = EntryPoints.sldcraft_abi_version();
                    if (abi != AbiVersion)
                        _loadError = "SolidCraft ABI mismatch: library reports version " + abi + ", this build expects " + AbiVersion;
                }
                catch (Exception e)
                {
                    _loadError = "cannot load the SolidCraft native library (" + string.Join(", ", Candidates()) + "): " + e.Message + "; set SOLIDCRAFT_FFI_LIB";
                }
            }
        }

        /// <summary>
        /// Verifies the shared-type layouts agree with the Rust library. Throws
        /// <see cref="SolidCraftNativeException"/> with a precise message on mismatch.
        /// </summary>
        public static void VerifyLayout()
        {
            if (!Available) throw new SolidCraftNativeException(LoadError ?? "the SolidCraft native library is unavailable");
            ulong[] sizes =
            {
                (ulong)Marshal.SizeOf(typeof(ScVec3)),
                (ulong)Marshal.SizeOf(typeof(ScFrame)),
                (ulong)Marshal.SizeOf(typeof(ScEntityRef)),
                (ulong)sizeof(ulong) * 2,            // ScStr: pointer + usize
                (ulong)Marshal.SizeOf(typeof(ScOwnedStr)),
                (ulong)Marshal.SizeOf(typeof(ScMassProperties)),
            };
            ScOwnedStr message = new ScOwnedStr();
            ScStatus status = EntryPoints.sldcraft_layout_guard(sizes, (UIntPtr)sizes.Length, ref message);
            string text = message.Take();
            if (status != ScStatus.Ok)
                throw new SolidCraftNativeException("SolidCraft type layout mismatch: " + (text ?? status.ToString()));
        }

        /// <summary>Entry points exactly as declared in the Rust ABI.</summary>
        public static class EntryPoints
        {
            [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
            public static extern int sldcraft_abi_version();

            [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
            public static extern IntPtr sldcraft_session_new();

            [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
            public static extern void sldcraft_session_free(IntPtr handle);

            [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
            public static extern void sldcraft_string_free(IntPtr s);

            [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
            public static extern void sldcraft_owned_str_free(ScOwnedStr s);

            [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
            public static extern ScStatus sldcraft_layout_guard(ulong[] sizes, UIntPtr n, ref ScOwnedStr message);

            // Command layer.
            [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
            public static extern IntPtr sldcraft_execute(IntPtr handle, IntPtr commandJson);

            [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
            public static extern IntPtr sldcraft_run_script(IntPtr handle, IntPtr scriptJson);

            [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
            public static extern IntPtr sldcraft_document_json(IntPtr handle);

            [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
            public static extern ScStatus sldcraft_load_document(IntPtr handle, IntPtr documentJson);

            // Typed layer.
            [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
            public static extern ScStatus sldcraft_measure(IntPtr handle, out ScMassProperties result);

            [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
            public static extern double sldcraft_total_volume_mm3(IntPtr handle);

            [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
            public static extern ScStatus sldcraft_body_count(IntPtr handle, out ulong count);

            [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
            public static extern ScStatus sldcraft_body_faces(IntPtr handle, IntPtr bodyUtf8, UIntPtr bodyLen, [Out] ScEntityRef[] outRefs, UIntPtr cap, out UIntPtr written);

            [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
            public static extern ScStatus sldcraft_body_edges(IntPtr handle, IntPtr bodyUtf8, UIntPtr bodyLen, [Out] ScEntityRef[] outRefs, UIntPtr cap, out UIntPtr written);

            [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
            public static extern ScStatus sldcraft_edge_endpoints(IntPtr handle, ScEntityRef edge, [Out] ScVec3[] outPoints);

            [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
            public static extern ScStatus sldcraft_parameter(IntPtr handle, IntPtr nameUtf8, UIntPtr nameLen, out double value);

            [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
            public static extern ScStatus sldcraft_sketch_rectangle(IntPtr handle, ScVec3 p0, ScVec3 p1, [Out] ScEntityRef[] outEdges, UIntPtr cap, out UIntPtr written);

            [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
            public static extern ScStatus sldcraft_sketch_dimension(IntPtr handle, ScEntityRef entity, double value);

            [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
            public static extern ScStatus sldcraft_extrude(IntPtr handle, double distanceMm, [MarshalAs(UnmanagedType.I1)] bool cut, out ulong featureId);
        }

        /// <summary>Small helpers over the entry points used by the higher-level bridge.</summary>
        public static class Invoke
        {
            internal static void OwnedStrFree(ScOwnedStr s) { EntryPoints.sldcraft_owned_str_free(s); }

            internal static string Execute(IntPtr handle, string json)
            {
                IntPtr arg = Marshal.StringToCoTaskMemUTF8(json ?? "");
                try { return Return(EntryPoints.sldcraft_execute(handle, arg)); }
                finally { Marshal.FreeCoTaskMem(arg); }
            }

            internal static string RunScript(IntPtr handle, string json)
            {
                IntPtr arg = Marshal.StringToCoTaskMemUTF8(json ?? "");
                try { return Return(EntryPoints.sldcraft_run_script(handle, arg)); }
                finally { Marshal.FreeCoTaskMem(arg); }
            }

            internal static string DocumentJson(IntPtr handle) { return Return(EntryPoints.sldcraft_document_json(handle)); }

            internal static ScStatus LoadDocument(IntPtr handle, string json)
            {
                IntPtr arg = Marshal.StringToCoTaskMemUTF8(json ?? "");
                try { return EntryPoints.sldcraft_load_document(handle, arg); }
                finally { Marshal.FreeCoTaskMem(arg); }
            }

            internal static ScStatus Parameter(IntPtr handle, string name, out double value)
            {
                byte[] bytes = System.Text.Encoding.UTF8.GetBytes(name ?? "");
                IntPtr ptr = bytes.Length == 0 ? IntPtr.Zero : Marshal.AllocCoTaskMem(bytes.Length);
                try
                {
                    if (bytes.Length > 0) Marshal.Copy(bytes, 0, ptr, bytes.Length);
                    return EntryPoints.sldcraft_parameter(handle, ptr, (UIntPtr)bytes.Length, out value);
                }
                finally { if (ptr != IntPtr.Zero) Marshal.FreeCoTaskMem(ptr); }
            }

            internal static ScStatus BodyFaces(IntPtr handle, string body, ScEntityRef[] outRefs, UIntPtr cap, out UIntPtr written)
            {
                UIntPtr w = UIntPtr.Zero;
                ScStatus status = WithUtf8(body, (ptr, len) => EntryPoints.sldcraft_body_faces(handle, ptr, len, outRefs, cap, out w));
                written = w;
                return status;
            }

            internal static ScStatus BodyEdges(IntPtr handle, string body, ScEntityRef[] outRefs, UIntPtr cap, out UIntPtr written)
            {
                UIntPtr w = UIntPtr.Zero;
                ScStatus status = WithUtf8(body, (ptr, len) => EntryPoints.sldcraft_body_edges(handle, ptr, len, outRefs, cap, out w));
                written = w;
                return status;
            }

            /// <summary>Passes a UTF-8 buffer as a Rust <c>ScStr</c> (ptr + len).</summary>
            private static ScStatus WithUtf8(string text, Func<IntPtr, UIntPtr, ScStatus> call)
            {
                byte[] bytes = System.Text.Encoding.UTF8.GetBytes(text ?? "");
                IntPtr ptr = bytes.Length == 0 ? IntPtr.Zero : Marshal.AllocCoTaskMem(bytes.Length);
                try
                {
                    if (bytes.Length > 0) Marshal.Copy(bytes, 0, ptr, bytes.Length);
                    return call(ptr, (UIntPtr)bytes.Length);
                }
                finally { if (ptr != IntPtr.Zero) Marshal.FreeCoTaskMem(ptr); }
            }

            private static string Return(IntPtr ptr)
            {
                if (ptr == IntPtr.Zero) return null;
                try { return Marshal.PtrToStringUTF8(ptr); }
                finally { EntryPoints.sldcraft_string_free(ptr); }
            }
        }
    }
}
