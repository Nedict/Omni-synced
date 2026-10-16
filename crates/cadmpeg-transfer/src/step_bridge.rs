//! In-memory STEP bridge: CadIr → STEP bytes → kernel STEP import → portable B-rep bodies.
//!
//! Callers persist CadIr only; this module is the sole place the bridge runs until direct
//! IR→portable mapping replaces it.

use cadmpeg_codec_step::StepCodec;
use cadmpeg_core::CodecError;
use cadmpeg_ir::codec::write::target::TargetRequest;
use cadmpeg_ir::codec::write::{EncodeInput, Encoder};
use cadmpeg_ir::report::Severity;
use cadmpeg_ir::CadIr;
use solidcraft_kernel::{import_portable_brep, portable_brep, ImportedBody, KernelError, StepImport};

use crate::TransferReport;

#[derive(Debug, thiserror::Error)]
pub enum TransferError {
    #[error("CadIr STEP export failed: {0}")]
    Export(String),
    #[error("CadIr transfer failed: {0}")]
    Import(#[from] KernelError),
    #[error("{0}")]
    Codec(#[from] CodecError),
}

fn step_bytes_from_ir(ir: &CadIr, warnings: &mut Vec<String>) -> Result<Vec<u8>, TransferError> {
    let plan = StepCodec::default().plan(EncodeInput::new(ir, None), TargetRequest::Inherit)?;
    for loss in &plan.report().losses {
        if matches!(loss.severity, Severity::Warning | Severity::Info) {
            warnings.push(loss.message.clone());
        }
    }
    let mut step = Vec::new();
    plan.write_to(&mut step).map_err(|e| TransferError::Export(e.to_string()))?;
    Ok(step)
}

fn step_text_from_ir(ir: &CadIr, warnings: &mut Vec<String>) -> Result<String, TransferError> {
    let bytes = step_bytes_from_ir(ir, warnings)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// STEP import preview (names, assembly tree) without rebuilding through portable B-rep.
pub fn step_import_from_ir(ir: &CadIr) -> Result<(StepImport, TransferReport), TransferError> {
    let mut warnings = Vec::new();
    let text = step_text_from_ir(ir, &mut warnings)?;
    let imp = solidcraft_kernel::step_import_shared(&text)?;
    warnings.extend(imp.warnings.iter().cloned());
    Ok(((*imp).clone(), TransferReport { warnings }))
}

/// Import bodies rebuilt from portable B-rep (same tree as STEP import).
pub fn portable_import_from_ir(ir: &CadIr) -> Result<(StepImport, TransferReport), TransferError> {
    let (preview, report) = step_import_from_ir(ir)?;
    let mut bodies = Vec::with_capacity(preview.bodies.len());
    for imported in &preview.bodies {
        let brep = portable_brep(&imported.body)?;
        let body = import_portable_brep(brep)?;
        bodies.push(ImportedBody {
            name: imported.name.clone(),
            body,
            color: imported.color,
            path: imported.path.clone(),
            instance: imported.instance.clone(),
            closed: imported.closed,
            file_faces: imported.file_faces,
            source_item: imported.source_item,
            face_origin: imported.face_origin.clone(),
        });
    }
    Ok((
        StepImport {
            bodies,
            tree: preview.tree,
            warnings: preview.warnings,
            schema: preview.schema,
            system: preview.system,
        },
        report,
    ))
}
