// SolidCraft C#-drives-Rust conformance harness.
//
// Loads the real solidcraft-ffi cdylib through the typed bridge and runs the workflows the
// ModelForge C# workspace performs, asserting the same outcomes. Every value is checked against
// an analytic figure, so a regression in either the Rust ABI or the C# marshalling fails the run.
//
// Run: `cargo xtask csharp` (builds the cdylib first), or directly with SOLIDCRAFT_FFI_LIB set.
using System;
using System.Collections.Generic;
using SolidCraft.Interop;

static class Program
{
    private static int _passed, _failed;

    private static void Check(bool ok, string what)
    {
        if (ok) { _passed++; Console.WriteLine("  PASS " + what); }
        else { _failed++; Console.WriteLine("  FAIL " + what); }
    }

    private static void Near(double got, double want, double tol, string what)
    {
        bool ok = Math.Abs(got - want) <= tol;
        Check(ok, what + " (got " + got.ToString("R") + ", want " + want.ToString("R") + ")");
    }

    private static int Main()
    {
        Console.WriteLine("SolidCraft C#-drives-Rust conformance");
        if (!Native.Available)
        {
            Console.WriteLine("  SKIP native library unavailable: " + Native.LoadError);
            return 2;
        }
        Console.WriteLine("  library: " + Native.ResolvedPath);

        // 1. Type layout: the shared structs must agree with Rust.
        try { Native.VerifyLayout(); Check(true, "shared type layouts agree with Rust"); }
        catch (Exception e) { Check(false, "shared type layouts agree with Rust: " + e.Message); }

        // 2. The primary ModelForge part workflow: rectangle -> dimension -> extrude -> measure.
        PlateWithHole();

        // 3. Hostile input must be rejected with a typed status, never a crash.
        HostileInput();

        // 4. Layout guard actually detects a mismatch.
        LayoutGuardDetectsMismatch();

        Console.WriteLine();
        Console.WriteLine(_failed == 0
            ? "SolidCraft C#-drives-Rust: " + _passed + " checks passed"
            : "SolidCraft C#-drives-Rust: " + _failed + " FAILED, " + _passed + " passed");
        return _failed == 0 ? 0 : 1;
    }

    /// <summary>The workspace-feature-test workflow: a 50x30x10 mm plate with an r=5 through hole.</summary>
    private static void PlateWithHole()
    {
        Console.WriteLine("workflow: plate with through hole (workspace-feature-test.cs)");
        using (SolidCraftSession s = new SolidCraftSession())
        {
            // SketchManager.InsertSketch on the Top Plane.
            s.Execute("sketch.create", "plane", "XY");
            ScEntityRef[] rect = s.SketchRectangle(0, 0, 50, 30);
            Check(rect.Length == 4, "corner rectangle returns 4 edges");
            s.Execute("sketch.finish");

            ulong boss = s.Extrude(10, false);
            Check(boss > 0, "extrusion creates a feature");
            Near(s.TotalVolumeMm3, 15000.0, 1e-6, "plate volume 50*30*10");

            // A second sketch with an r=5 circle, cut through.
            s.Execute("sketch.create", "plane", "XY");
            s.Execute("sketch.circle.center", "center", new double[] { 25, 15 }, "radius", 5.0);
            s.Execute("sketch.finish");
            ulong cut = s.Extrude(10, true);
            Check(cut > 0, "cut creates a feature");

            double want = 15000.0 - Math.PI * 25.0 * 10.0;
            Near(s.TotalVolumeMm3, want, 1e-6, "plate minus through hole");

            // Typed mass properties agree with the scalar volume.
            ScMassProperties mp = s.Measure();
            Near(mp.VolumeMm3, want, 1e-6, "measure() volume matches");
            Check(mp.Faces > 0 && mp.Edges > 0, "measure() reports faces and edges");

            // Body/face/edge enumeration is typed.
            Check(s.BodyCount() == 1, "one body");
            ScEntityRef[] edges = s.BodyEdges("");
            Check(edges.Length > 0, "edges enumerated (" + edges.Length + ")");
            Check(edges[0].Kind == ScEntityKind.Edge, "edge references carry the Edge kind");
        }
    }

    private static void HostileInput()
    {
        Console.WriteLine("hostile input is rejected, not crashed");
        using (SolidCraftSession s = new SolidCraftSession())
        {
            bool threw = false;
            try { s.Extrude(double.NaN, false); }
            catch (SolidCraftCommandException) { threw = true; }
            Check(threw, "NaN extrude distance is refused");

            threw = false;
            try { s.Execute("no.such.command"); }
            catch (SolidCraftCommandException) { threw = true; }
            Check(threw, "unknown command is refused");

            Check(s.TryExecute("no.such.command") == null, "TryExecute returns null instead of throwing");

            double v;
            Check(!s.TryParameter("does_not_exist", out v), "missing parameter reported");
        }
    }

    private static void LayoutGuardDetectsMismatch()
    {
        Console.WriteLine("layout guard detects a deliberate mismatch");
        // A wrong size for ScVec3 must be reported, not accepted.
        ulong[] bad = { 8, 72, 24, 16, 16, 128 };
        ScOwnedStr message = new ScOwnedStr();
        ScStatus status = Native.EntryPoints.sldcraft_layout_guard(bad, (UIntPtr)bad.Length, ref message);
        string text = message.Take();
        Check(status != ScStatus.Ok && text != null && text.Contains("ScVec3"), "guard names the offending type: " + text);
    }
}
