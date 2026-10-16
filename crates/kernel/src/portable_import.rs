//! Portable B-rep (`solidcraft_geom::brep`) import without STEP text.
use std::sync::Arc;

use solidcraft_geom::brep as g;

use crate::native;
use crate::{Body, KernelError, Result, guard};

fn invalid(detail: impl std::fmt::Display) -> KernelError {
    KernelError::Invalid(format!("portable B-rep: {detail}"))
}

/// Encode a kernel body as portable B-rep (for neutral transfer and native cache).
pub fn portable_brep(body: &Body) -> Result<g::Brep> {
    if body.mesh.is_some() {
        return Err(invalid("mesh bodies have no B-rep"));
    }
    guard("portable B-rep encode", || native::encode_portable(&body.solid))
}

/// Build a kernel body from validated portable B-rep (import path).
pub fn import_portable_brep(brep: g::Brep) -> Result<Body> {
    brep.validate().map_err(invalid)?;
    guard("portable B-rep import", || {
        let solid = native::decode_portable(brep)?;
        Ok(Body {
            solid: Arc::new(solid),
            size: 0.0,
            volume: Some(Arc::new(std::sync::Mutex::new(None))),
            coarse_missing: None,
            mesh: None,
            color: None,
            paint: None,
            splits: None,
            face_sources: None,
            history: None,
            display_of: None,
            regularization_protected: false,
        })
    })
}
