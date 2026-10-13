// The `ModelForge.Interop.sldworks` interface contract the ModelForge C# workspace compiles
// against, re-declared for SolidCraft.
//
// The workspace source is unchanged: it does `using ModelForge.Interop.sldworks;` and calls these
// members exactly as it does against SolidWorks. This file provides the *declarations*; the
// implementations live in `sldworks/Impl*.cs` and are backed by the SolidCraft typed bridge
// (SolidCraft.Interop) instead of COM.
//
// Only the members the workspace actually uses are declared, with the signatures those call sites
// require (verified against the workspace sources in the ModelForge snapshot). Members that are
// declared but not yet implemented throw NotSupportedException from the implementation — never a
// crash — and are listed in docs/modelforge-interop.md.
using System;

namespace ModelForge.Interop.sldworks
{
    // Note: there is deliberately no `DispatchWrapper` declared here. The workspace wraps
    // feature-data objects in `System.Runtime.InteropServices.DispatchWrapper` (its COM origin),
    // so re-declaring the name would make every use an ambiguous reference.

    /// <summary>Selection data handed to <c>Select4</c>. Mirrors the COM <c>SelectData</c> class.</summary>
    public class SelectData
    {
        public int Mark { get; set; }
        public string Callout { get; set; }
    }

    public interface IFeatureManager
    {
        IFeature FeatureExtrusion2(bool singleEnded, bool flip, bool direction, int endCondition, int endCondition2, double depth, double depth2, bool draft, bool draft2, bool draftInward, bool draftInward2, double draftAngle, double draftAngle2, bool offsetReverse, bool offsetReverse2, bool translateSurface, bool translateSurface2, bool merge, bool useFeatScope, bool useAutoSelect, int startCondition, int startOffset, bool startOffsetReverse);
        IFeature FeatureCut3(bool singleEnded, bool flip, bool direction, int endCondition, int endCondition2, double depth, double depth2, bool draft, bool draft2, bool draftInward, bool draftInward2, double draftAngle, double draftAngle2, bool offsetReverse, bool offsetReverse2, bool translateSurface, bool translateSurface2, bool normalCut, bool useFeatScope, bool useAutoSelect, bool assemblyFeatureScope, bool autoSelectComponents, bool propagateFeatureToParts, int startCondition, int startOffset, bool startOffsetReverse);
        IFeature FeatureFillet(int options, double radius, int setBackType, double setBack, object pointRadius, object edgeRadius, object setBackRadius);
        IFeature InsertFeatureChamfer(int options, int chamferType, double width, double angle, double otherDistance, double otherAngle, double depth, double depth2);
        IFeature InsertProtrusionBlend(bool close, bool keepTangency, bool forceNonRational, double tessToleranceFactor, bool startMatchingType, bool endMatchingType, int sections, int sections2, bool thinWall, double thinWallThickness, int reverse, int reverse2, bool merge, bool useFeatScope, bool useAutoSelect, int startCondition, int startOffset, bool startOffsetReverse, int endCondition, int endOffset, bool endOffsetReverse, double tangencyType, double tangencyType2);
    }

    public interface IEntity
    {
        IComponent2 GetComponent();
        bool Select4(bool append, SelectData data);
        bool Select4(bool append, SelectData data, bool mark);
    }

    public interface ISelectData
    {
        int Mark { get; set; }
        string Callout { get; set; }
    }

    public interface ISelectionMgr
    {
        ISelectData CreateSelectData();
        int GetSelectedObjectCount2(int mark);
        object GetSelectedObject6(int index, int mark);
        bool ClearSelection2(bool all);
    }

    public interface IMathUtility
    {
        IMathPoint CreatePoint(object point);
        IMathVector CreateVector(object vector);
    }

    public interface IMathPoint
    {
        object[] ArrayData { get; set; }
        IMathPoint MultiplyTransform(IMathTransform transform);
    }

    public interface IMathVector
    {
        object[] ArrayData { get; set; }
        IMathVector MultiplyTransform(IMathTransform transform);
    }

    public interface IMathTransform
    {
        object[] ArrayData { get; set; }
    }

    public interface IMassProperty
    {
        double Volume { get; }
        double SurfaceArea { get; }
        double Mass { get; }
        double[] CenterOfMass { get; }
    }

    public interface IVertex
    {
        double[] GetPoint();
    }

    public interface ICurve
    {
        bool IsLine();
        bool IsCircle();
        double[] GetEndParams();
        double[] Evaluate2(double parameter, int derivative);
        object GetTessPts(double startTol, double endTol, object startPoint, object endPoint);
        double[] CircleParams { get; }
        double[] LineParams { get; }
    }

    public interface ISurface
    {
        bool IsPlane();
        bool IsCylinder();
        double[] PlaneParams { get; }
        double[] CylinderParams { get; }
    }

    public interface IBody2
    {
        string Name { get; set; }
        object[] GetFaces();
        object[] GetEdges();
        IMassProperty GetMassProperties();
        ITessellation GetTessellation();
    }

    public interface IFace2
    {
        double GetArea();
        object GetBox();
        IFeature GetFeature();
        ISurface GetSurface();
        object GetTessTriangles(bool noConversion);
        double[] Normal { get; }
    }

    public interface IEdge
    {
        ICurve GetCurve();
        double[] GetCurveParams2();
        IVertex GetStartVertex();
        IVertex GetEndVertex();
    }

    public interface ITessellation
    {
        bool NeedFaceFacetMap { get; set; }
        bool NeedVertexNormal { get; set; }
        void Tessellate();
        object[] GetFaceFacets(IFace2 face);
        object[] GetFacetFins(int facet);
        object[] GetFinVertices(int fin);
        double[] GetVertexNormal(int vertex);
        double[] GetVertexPoint(int vertex);
    }

    public interface ISketchPoint
    {
        double X { get; }
        double Y { get; }
        double Z { get; }
        int GetID();
        bool Select(bool append, int mark);
        void SetCoords(double x, double y, double z);
    }

    public interface ISketchSegment
    {
        int GetID();
        int GetType();
        bool Is3D();
        bool ConstructionGeometry { get; set; }
        bool Construction { get; set; }
        double GetLength();
        double[] GetCurve();
        object GetSketchPoints2();
        object GetSketchSegments();
        int GetConstrainedStatus();
        bool Select4(bool append, SelectData data);
        ISketchRelationManager RelationManager { get; }
    }

    public interface ISketchLine : ISketchSegment
    {
        ISketchPoint GetStartPoint2();
        ISketchPoint GetEndPoint2();
    }

    public interface ISketchArc : ISketchSegment
    {
        ISketchPoint GetStartPoint2();
        ISketchPoint GetEndPoint2();
        ISketchPoint GetCenterPoint2();
        double GetRadius();
        int GetRotationDir();
        bool IsCircle();
    }

    public interface ISketchRelation
    {
        int GetRelationType();
        object GetDefinitionEntities();
        object GetEntities();
        bool Suppressed { get; set; }
        int Type { get; }
        object Entities { get; }
    }

    public interface ISketchRelationManager
    {
        object GetRelations(int filter);
        int GetRelationsCount(int filter);
        object AddRelation(object entities, int relationType);
        bool DeleteRelation(object relation);
        bool AddToDB { get; set; }
        object CreateCornerRectangle(double x1, double y1, double z1, double x2, double y2, double z2);
        object CreateLine(double x1, double y1, double z1, double x2, double y2, double z2);
        object CreateCircleByRadius(double cx, double cy, double cz, double radius);
        object CreateArc(double xc, double yc, double zc, double x1, double y1, double z1, double x2, double y2, double z2, int direction);
    }

    public interface ISketch
    {
        bool Is3D();
        object GetSketchSegments();
        object GetReferenceEntity();
        ISketchRelationManager RelationManager { get; }
        IMathTransform ModelToSketchTransform { get; }
    }

    public interface ISketchManager
    {
        void InsertSketch(bool updateEditRebuild);
        object CreateCornerRectangle(double x1, double y1, double z1, double x2, double y2, double z2);
        object CreateLine(double x1, double y1, double z1, double x2, double y2, double z2);
        object CreateCircleByRadius(double cx, double cy, double cz, double radius);
        object CreateArc(double xc, double yc, double zc, double x1, double y1, double z1, double x2, double y2, double z2, int direction);
        object SketchTrim(int option, object segment, double x, double y, double z);
        object SketchUseEdge2(bool chain);
    }

    public interface IDimension
    {
        string FullName { get; }
        string Name { get; }
        bool ReadOnly { get; }
        int GetType();
        double SystemValue { get; set; }
        double GetSystemValue3(int whichConfigurations, object configNames);
        int SetSystemValue3(double value, int whichConfigurations, object configNames);
        int DrivenState { get; }
    }

    public interface IDisplayDimension
    {
        IDimension GetDimension2(int index);
        bool ShowDialog();
    }

    public interface IConfiguration
    {
        string Name { get; }
        string Description { get; set; }
        bool UseAlternateNameInBOM { get; set; }
        string AlternateName { get; set; }
    }

    public interface IConfigurationManager
    {
        IConfiguration ActiveConfiguration { get; }
        object GetConfigurationByName(string name);
        object GetConfigurationNames();
        object AddConfiguration(string name, string comment, string alternateName, int options, string parent, string description, bool checkCompatibility);
    }

    public interface IEquationMgr
    {
        int GetCount();
        string get_Equation(int index);
        void set_Equation(int index, string equation);
    }

    public interface ICustomPropertyManager
    {
        object GetAll();
    }

    public interface IFeature
    {
        string Name { get; }
        string GetTypeName2();
        IFeature GetNextFeature();
        IFeature GetFirstSubFeature();
        IFeature GetNextSubFeature();
        object GetSpecificFeature2();
        object GetDefinition();
        object IGetDefinition();
        bool ModifyDefinition(object definition, object owner);
        bool Select2(bool append, int mark);
        int GetErrorCode2(out bool warning);
        object IsSuppressed();
        object IsSuppressed2(int whichConfigurations, object configNames);
        bool SetSuppression2(int action, int whichConfigurations, object configNames);
        int SetSuppression2(int state);
        object GetFirstDisplayDimension();
        object GetNextDisplayDimension(object displayDimension);
        object GetParents();
    }

    public interface IExtrudeFeatureData2
    {
        bool AccessSelections(object topDoc, object component);
        void ReleaseSelectionAccess();
        double GetDepth(bool forward);
        void SetDepth(bool forward, double depth);
        int GetEndCondition(bool forward);
        void SetEndCondition(bool forward, int endCondition);
        object GetEndConditionReference(bool forward);
        void SetEndConditionReference(bool forward, object reference);
    }

    public interface IRevolveFeatureData2
    {
        bool AccessSelections(object topDoc, object component);
        void ReleaseSelectionAccess();
    }

    public interface IChamferFeatureData2
    {
        bool AccessSelections(object topDoc, object component);
        void ReleaseSelectionAccess();
        double GetEdgeChamferDistance(int side);
        void SetEdgeChamferDistance(int side, double distance);
        double EdgeChamferAngle { get; set; }
        object Edges { get; set; }
    }

    public interface ISimpleFilletFeatureData2
    {
        bool AccessSelections(object topDoc, object component);
        void ReleaseSelectionAccess();
        double DefaultRadius { get; set; }
        object Edges { get; set; }
        int GetEndCondition();
        bool AutoSelect { get; set; }
        bool BothDirections { get; set; }
        bool ConstantWidth { get; set; }
        bool CurvatureContinuous { get; set; }
        bool FeatureScope { get; set; }
        int FromType { get; set; }
        bool IsMultipleRadius { get; set; }
        bool Merge { get; set; }
        bool OmitAttachedEdges { get; set; }
        int OverflowType { get; set; }
        bool ReverseDirection { get; set; }
        int ThinWallType { get; set; }
        int Type { get; set; }
    }

    public interface ILoftFeatureData
    {
        bool AccessSelections(object topDoc, object component);
        void ReleaseSelectionAccess();
        int GetGuideCurvesCount();
        int GetGuideCurvesType();
        int GetGuideTangencyType();
    }

    public interface IFillSurfaceFeatureData
    {
        object GetConstraintCurves();
        int GetCurvatureControl();
        object GetPatchBoundary();
    }

    public interface IAnnotation
    {
        bool SetPosition(double x, double y, double z);
    }

    public interface INote
    {
        IAnnotation GetAnnotation();
        string GetText();
    }

    public interface ITextFormat
    {
        bool Bold { get; set; }
        bool Italic { get; set; }
        double CharHeight { get; set; }
        string TypeFaceName { get; set; }
    }

    public interface IComponent2
    {
        string Name2 { get; }
        string GetPathName();
        IModelDoc2 GetModelDoc2();
        object GetBodies3(int bodyType, bool visibleOnly);
        object GetChildren();
        IComponent2 GetParent();
        IComponent2 GetCorresponding(object entity);
        object GetCorrespondingEntity(object entity);
        object GetSuppression();
        int SetSuppression2(int state);
        bool IsFixed();
        bool IsRoot();
        bool Select4(bool append, SelectData data, bool mark);
        bool SetTransformAndSolve2(IMathTransform transform);
        string ReferencedConfiguration { get; }
        IMathTransform Transform2 { get; }
        object Transform { get; }
    }

    public interface IAssemblyDoc
    {
        object GetComponents(bool topLevelOnly);
        int GetComponentCount(bool topLevelOnly);
        object AddComponent5(string path, int configOption, string configName, bool useConfigForInstance, string instanceName, double x, double y, double z);
        object AddMate3(int mateType, int align, bool flip, double distance, double distance2, double angle, double angle2, double gearRatioNumerator, double gearRatioDenominator, double gearRatio2Numerator, double gearRatio2Denominator, bool forPositioningOnly, out int errorStatus);
        bool EditMate2(object mate, int align, bool flip, double distance, double angle);
        bool UnfixComponent();
        bool FixComponent();
        bool ReplaceComponents2(string fileName, bool replaceAllInstances, bool useConfigForInstance, string configName, bool replaceAllComponents, bool matchByName, object component, out int status);
    }

    public interface IMateEntity2
    {
        object GetEntity();
    }

    public interface IMate2
    {
        int Type { get; }
        int Alignment { get; }
        int GetMateEntityCount();
        IMateEntity2 MateEntity(int index);
    }

    public interface IModelDocExtension
    {
        bool SelectByID2(string name, string type, double x, double y, double z, bool append, int mark, object callout, int selectOption);
        IMassProperty CreateMassProperty();
        object GetPersistReference3(object entity);
        object GetObjectByPersistReference3(object reference, out int error);
        int IsSamePersistentID(object first, object second);
        object GetCorresponding(object entity);
        object GetCorrespondingEntity(object entity);
        bool SaveAs(string fileName, int saveAsVersion, int saveAsOptions, object exportData, ref int errors, ref int warnings);
        object DeleteSelection2(bool all);
    }

    public interface IModelDoc2
    {
        IModelDocExtension Extension { get; }
        ISketchManager SketchManager { get; }
        IFeatureManager FeatureManager { get; }
        ISelectionMgr SelectionManager { get; }
        IConfigurationManager ConfigurationManager { get; }
        IModelView ActiveView { get; }
        IFeature ActiveFeature { get; }
        string GetPathName();
        string GetTitle();
        IFeature FirstFeature();
        object Parameter(string name);
        bool EditRebuild3();
        bool ForceRebuild3(bool topOnly);
        void ClearSelection2(bool all);
        ISketch GetActiveSketch2();
        bool EditSketch();
        bool EditPart2(string fileName, double x, double y, double z, int options, out int errors);
        bool EditAssembly(string fileName, bool useLastActive);
        IModelDoc2 GetEditTarget();
        IComponent2 GetEditTargetComponent();
        object AddDimension2(double x, double y, double z);
        bool InsertSketch(bool updateEditRebuild);
        object FeatureByName(string name);
        object AddConfiguration3(string name, string comment, string alternateName, int options);
        IModelDoc2 GetConfigurationByName(string name);
        object GetConfigurationNames();
        bool ShowConfiguration2(string name);
        IEquationMgr GetEquationMgr();
        object GetDependencies2(string fileName, bool traverseDependents, bool searchAssemblies, bool includeVirtual);
        object ListExternalFileReferences2(int count, out string fileName);
        int ListExternalFileReferencesCount2(bool searchCurrentDoc);
        bool Save3(int options, ref int errors, ref int warnings);
        INote InsertNote(string text);
    }

    public interface IModelView
    {
        IModelDoc2 GetModelDoc2();
        bool ZoomByFactor(double factor);
    }

    public interface IPartDoc
    {
        object GetBodies2(int bodyType, bool visibleOnly);
        IFeature FeatureByName(string name);
        string GetMaterialPropertyName2(string configName, out string database);
    }

    public interface IDrawingDoc
    {
        IFeature FirstFeature();
        ISheet GetCurrentSheet();
        IView GetFirstView();
        object GetSheetNames();
    }

    public interface ISheet
    {
        object GetSize(out double width, out double height);
    }

    public interface IView
    {
        string GetName2();
        IView GetNextView();
        object GetNotes();
        object GetOutline();
        string GetReferencedModelName();
        IMathTransform ModelToViewTransform { get; }
        string ReferencedConfiguration { get; }
        IModelDoc2 ReferencedDocument { get; }
    }

    public interface IFutureFeatureDataForContextProbe
    {
        bool AccessSelections(object topDoc, object component);
    }

    public interface ISldWorks
    {
        IModelDoc2 NewPart();
        IModelDoc2 NewAssembly();
        IModelDoc2 NewDocument(string templateName, int paperSize, double width, double height);
        IModelDoc2 OpenDoc6(string fileName, int type, int options, string configuration, ref int errors, ref int warnings);
        IModelDoc2 ActivateDoc2(string name, bool silent, ref int errors);
        bool CloseDoc(string name);
        IModelDoc2 GetOpenDocumentByName(string name);
        object GetDocumentDependencies2(string document, bool traverse, bool searchAssemblies, bool addReadOnlyInfo);
        bool ReplaceReferencedDocument(string document, string oldName, string newName);
        int GetProcessID();
        IMathUtility GetMathUtility();
        bool GetUserPreferenceToggle(int userPreference);
        void SetUserPreferenceToggle(int userPreference, bool option);
        string GetUserPreferenceStringValue(int userPreference);
    }
}
