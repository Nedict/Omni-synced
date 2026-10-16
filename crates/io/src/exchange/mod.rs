//! Exchange file providers: one registry for import/open metadata and decode.
//!
//! **Adding a format:** add a submodule with [`ExchangeFileType`] + `read`, register it in
//! [`providers`], extend [`ExchangeProviderId`], and add a arm in [`read_bytes`]. Call sites
//! (`doc.open`, insert commands, open dialogs, native-design sniff) should use this module.
//!
//! **Export / save:** design save stays on [`crate::DESIGN_EXT`] / JSON; mesh and B-rep export
//! stay on [`crate::Format`] until an export arm is added per provider.

mod mesh;
mod step;
#[cfg(not(target_arch = "wasm32"))]
mod fusion;

use std::path::Path;

use crate::{IoError, MeshFeature, Result, StepFeature};

pub use mesh::FILE_TYPE as MESH_FILE_TYPE;
pub use step::FILE_TYPE as STEP_IGES_FILE_TYPE;
#[cfg(not(target_arch = "wasm32"))]
pub use fusion::FILE_TYPE as FUSION_F3D_FILE_TYPE;

/// Stable id for logging, tests and future MCP metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ExchangeProviderId {
    StepIges,
    #[cfg(not(target_arch = "wasm32"))]
    FusionF3d,
    MeshStl3mf,
}

/// What this format supports in SolidCraft today.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExchangeCapabilities {
    pub open_new_design: bool,
    pub insert_bodies: bool,
}

/// Static metadata shared by UI filters, vfs limits and engine commands.
#[derive(Clone, Copy, Debug)]
pub struct ExchangeFileType {
    pub id: ExchangeProviderId,
    pub dialog_label: &'static str,
    pub extensions: &'static [&'static str],
    pub max_bytes: usize,
    pub capabilities: ExchangeCapabilities,
}

/// Every registered exchange provider (platform-specific rows omitted on wasm).
pub fn providers() -> &'static [ExchangeFileType] {
    #[cfg(not(target_arch = "wasm32"))]
    {
        &[step::FILE_TYPE, fusion::FILE_TYPE, mesh::FILE_TYPE]
    }
    #[cfg(target_arch = "wasm32")]
    {
        &[step::FILE_TYPE, mesh::FILE_TYPE]
    }
}

pub fn provider(id: ExchangeProviderId) -> &'static ExchangeFileType {
    for p in providers() {
        if p.id == id {
            return p;
        }
    }
    // Every ExchangeProviderId variant is registered in `providers()`.
    &step::FILE_TYPE
}

fn extension_of(path: &str) -> Option<&str> {
    Path::new(path).extension()?.to_str()
}

fn extension_matches(ext: &str, allowed: &[&str]) -> bool {
    allowed.iter().any(|a| ext.eq_ignore_ascii_case(a))
}

pub fn provider_for_path(path: &str) -> Option<ExchangeProviderId> {
    let ext = extension_of(path)?;
    providers().iter().find(|p| extension_matches(ext, p.extensions)).map(|p| p.id)
}

pub fn matches_path(id: ExchangeProviderId, path: &str) -> bool {
    provider_for_path(path) == Some(id)
}

/// Bodies produced when opening or inserting an exchange file.
pub enum ExchangeImport {
    Solid(StepFeature),
    Mesh(MeshFeature),
}

pub fn read_path(path: &str) -> Result<ExchangeImport> {
    let id = provider_for_path(path).ok_or_else(|| IoError::Format(path.to_string()))?;
    let max = provider(id).max_bytes;
    let bytes = crate::vfs::read(path, max).map_err(|e| IoError::Invalid(format!("{path}: {e}")))?;
    read_bytes(id, &bytes, path)
}

pub fn read_bytes(id: ExchangeProviderId, bytes: &[u8], path: &str) -> Result<ExchangeImport> {
    match id {
        ExchangeProviderId::StepIges => step::read(bytes, path),
        #[cfg(not(target_arch = "wasm32"))]
        ExchangeProviderId::FusionF3d => fusion::read(bytes, path),
        ExchangeProviderId::MeshStl3mf => mesh::read(bytes, path),
    }
}

pub fn opens_new_design(path: &str) -> bool {
    provider_for_path(path).is_some_and(|id| provider(id).capabilities.open_new_design)
}

pub fn can_insert(path: &str) -> bool {
    provider_for_path(path).is_some_and(|id| provider(id).capabilities.insert_bodies)
}

pub fn refuse_native_design(bytes: &[u8]) -> Option<IoError> {
    #[cfg(not(target_arch = "wasm32"))]
    if fusion::blocks_native_design(bytes) {
        return Some(crate::fusion_design_refusal());
    }
    #[cfg(target_arch = "wasm32")]
    if crate::looks_like_fusion_archive(bytes) {
        return Some(crate::fusion_design_refusal());
    }
    None
}

pub fn open_dialog_combined_extensions() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    let mut push = |e: &'static str| {
        if !out.iter().any(|x| x.eq_ignore_ascii_case(e)) {
            out.push(e);
        }
    };
    push(crate::DESIGN_EXT);
    push(crate::LEGACY_DESIGN_EXT);
    push("json");
    for p in providers() {
        for e in p.extensions {
            push(e);
        }
    }
    out
}

pub fn insert_command_name(path: &str) -> Option<&'static str> {
    match provider_for_path(path)? {
        ExchangeProviderId::MeshStl3mf => Some("file.insert_mesh"),
        ExchangeProviderId::StepIges => Some("file.insert_step"),
        #[cfg(not(target_arch = "wasm32"))]
        ExchangeProviderId::FusionF3d => Some("file.insert_step"),
    }
}

pub fn opens_as_step_or_fusion_insert(path: &str) -> bool {
    match provider_for_path(path) {
        Some(ExchangeProviderId::StepIges) => true,
        #[cfg(not(target_arch = "wasm32"))]
        Some(ExchangeProviderId::FusionF3d) => true,
        _ => false,
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn opens_as_fusion_insert(path: &str) -> bool {
    provider_for_path(path) == Some(ExchangeProviderId::FusionF3d)
}

pub fn dialog_filters() -> Vec<(&'static str, &'static [&'static str])> {
    let mut out = Vec::new();
    out.push(("STEP", &["step", "stp", "STEP", "STP"] as &[&str]));
    out.push(("IGES", &["igs", "iges", "IGS", "IGES"]));
    for p in providers() {
        if p.id == ExchangeProviderId::StepIges {
            continue;
        }
        out.push((p.dialog_label, p.extensions));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_for_path_extensions() {
        assert_eq!(provider_for_path("/a/part.STEP"), Some(ExchangeProviderId::StepIges));
        assert_eq!(provider_for_path("x.stp"), Some(ExchangeProviderId::StepIges));
        assert!(!matches_path(ExchangeProviderId::StepIges, "a.step.json"));
        assert_eq!(provider_for_path("m.stl"), Some(ExchangeProviderId::MeshStl3mf));
        assert_eq!(provider_for_path("m.obj"), Some(ExchangeProviderId::MeshStl3mf));
        #[cfg(not(target_arch = "wasm32"))]
        assert_eq!(provider_for_path("f.F3Z"), Some(ExchangeProviderId::FusionF3d));
        assert_eq!(provider_for_path("design.solidcraft"), None);
    }

    #[test]
    fn insert_command_routing() {
        assert_eq!(insert_command_name("a.step"), Some("file.insert_step"));
        assert_eq!(insert_command_name("a.stl"), Some("file.insert_mesh"));
        #[cfg(not(target_arch = "wasm32"))]
        assert_eq!(insert_command_name("a.f3d"), Some("file.insert_step"));
    }
}
