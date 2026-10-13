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
using ModelForge.Interop.sldworks;
using ModelForge.Interop.swconst;

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

    /// <summary>
    /// Relative comparison. Curved features (a cut by a cylindrical hole) are measured from the
    /// tessellated mesh, so the analytic area differs by the polygon's chord error; use a small
    /// relative tolerance rather than an absolute one.
    /// </summary>
    private static void NearRel(double got, double want, double relTol, string what)
    {
        bool ok = Math.Abs(got - want) <= Math.Abs(want) * relTol;
        Check(ok, what + " (got " + got.ToString("R") + ", want " + want.ToString("R") + ", rel " + (Math.Abs(got - want) / Math.Abs(want)).ToString("E2") + ")");
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

        // 5. The same workflow written the way the ModelForge workspace writes it, against the
        //    SolidWorks interfaces (ISldWorks/IModelDoc2/ISketchManager/IFeatureManager).
        SolidWorksInterfaceWorkflow();

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
            NearRel(s.TotalVolumeMm3, want, 1e-4, "plate minus through hole");

            // Typed mass properties agree with the scalar volume.
            ScMassProperties mp = s.Measure();
            NearRel(mp.VolumeMm3, want, 1e-4, "measure() volume matches");
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

    /// <summary>
    /// The workspace's part workflow written against the SolidWorks interfaces — the same calls
    /// `solidworks-adapter.cs` makes — but backed by SolidCraft. This proves the C# that drove
    /// SolidWorks now drives Rust.
    /// </summary>
    private static void SolidWorksInterfaceWorkflow()
    {
        Console.WriteLine("workflow: SolidWorks interfaces (ISldWorks/IModelDoc2/IFeatureManager)");
        Application app = new Application();
        IModelDoc2 doc = app.NewPart();
        Check(doc != null, "NewPart returns a document");

        // SelectByID2 + InsertSketch, as the adapter does for the Top Plane.
        doc.Extension.SelectByID2("Top Plane", "PLANE", 0, 0, 0, false, 0, null, 0);
        doc.SketchManager.InsertSketch(true);

        doc.SketchManager.CreateCornerRectangle(0, 0, 0, 0.05, 0.03, 0);
        doc.SketchManager.InsertSketch(true);   // finish the sketch

        // Extrude 10 mm (SolidWorks: 0.01 m) with FeatureExtrusion2.
        doc.ClearSelection2(true);
        IFeature boss = doc.FeatureManager.FeatureExtrusion2(true, false, false, (int)swEndConditions_e.swEndCondBlind, 0, 0.01, 0.01, false, false, false, false, 0, 0, false, false, false, false, true, true, true, 0, 0, false);
        Check(boss != null, "FeatureExtrusion2 returns a feature");
        Check(doc.EditRebuild3(), "EditRebuild3 succeeds");

        // Mass properties, in SolidWorks units (m³).
        IMassProperty mp = doc.Extension.CreateMassProperty();
        double volumeM3 = mp != null ? mp.Volume : double.NaN;
        Near(volumeM3, 0.05 * 0.03 * 0.01, 1e-12, "mass property volume in m^3");

        // The document object model: features and bodies.
        IFeature first = doc.FirstFeature();
        Check(first != null, "FirstFeature returns a feature");
        object[] bodies = ((IPartDoc)doc).GetBodies2(0, false) as object[];
        Check(bodies != null && bodies.Length == 1, "GetBodies2 returns one body");

        // A persist reference round-trips through the extension, and names a body.
        object reference = doc.Extension.GetPersistReference3(bodies[0]);
        int error;
        object resolved = doc.Extension.GetObjectByPersistReference3(reference, out error);
        Check(error == 0 && resolved is IBody2, "persist reference resolves back to a body");
        Check(doc.Extension.IsSamePersistentID(reference, doc.Extension.GetPersistReference3(bodies[0])) == (int)swObjectEquality.swObjectSame, "IsSamePersistentID agrees for the same entity");

        // A through cut, as the workspace makes with FeatureCut3, and its analytic volume.
        doc.SketchManager.InsertSketch(true);
        doc.SketchManager.CreateCircleByRadius(0.025, 0.015, 0, 0.005);
        doc.SketchManager.InsertSketch(true);
        doc.ClearSelection2(true);
        IFeature cut = doc.FeatureManager.FeatureCut3(true, false, false, (int)swEndConditions_e.swEndCondBlind, 0, 0.01, 0.01, false, false, false, false, 0, 0, false, false, false, false, false, false, true, false, false, false, 0, 0, false);
        Check(cut != null, "FeatureCut3 returns a feature");
        NearRel(doc.Extension.CreateMassProperty().Volume, 0.05 * 0.03 * 0.01 - Math.PI * 0.005 * 0.005 * 0.01, 1e-4, "plate minus through hole (m^3)");

        // An edge fillet with FeatureFillet.
        object[] partBodies = (object[])((IPartDoc)doc).GetBodies2(0, false);
        object[] edges = ((IBody2)partBodies[0]).GetEdges() as object[];
        Check(edges != null && edges.Length > 0, "body exposes edges");
        ((IEdge)edges[0]).Select4(true, null);
        IFeature fillet = doc.FeatureManager.FeatureFillet((int)swFeatureFilletOptions_e.swFeatureFilletUniformRadius, 0.001, 0, 0, null, null, null);
        Check(fillet != null, "FeatureFillet returns a feature");

        // Feature suppression round-trips, and a persist reference taken before a rebuild still
        // resolves after it (the IsSamePersistentID contract).
        object featureRef = doc.Extension.GetPersistReference3(fillet);
        fillet.SetSuppression2((int)swFeatureSuppressionAction_e.swSuppressFeature, (int)swInConfigurationOpts_e.swThisConfiguration, null);
        doc.EditRebuild3();
        fillet.SetSuppression2((int)swFeatureSuppressionAction_e.swUnSuppressFeature, (int)swInConfigurationOpts_e.swThisConfiguration, null);
        doc.EditRebuild3();
        int pinError;
        object pinResolved = doc.Extension.GetObjectByPersistReference3(featureRef, out pinError);
        Check(pinError == 0 && pinResolved is IFeature, "a feature persist reference survives suppression and rebuild");

        app.CloseDoc(doc.GetTitle());
        Check(app.GetOpenDocumentByName(doc.GetTitle()) == null, "CloseDoc releases the document");
    }
}
