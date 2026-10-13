//! SolidCraft MCP server.
//!
//! JSON-RPC 2.0 over stdio (newline-delimited), implementing the MCP lifecycle, tools and
//! resources. Tools map onto the control-channel methods (`engine.execute`, `document.inspect`,
//! `ui.render`…, see `docs/control-protocol.md`), served either by an in-process headless
//! [`Headless`] session or by a running app through its loopback control port ([`Remote`]), so a
//! person can watch the agent model. Every modelling step is an engine command; tools never open
//! dialogs.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![forbid(unsafe_code)]

mod backend;
mod server;
mod tools;

pub use backend::{Backend, Headless, Remote};
pub use server::{PROTOCOL_VERSION, SUPPORTED_VERSIONS, Server};
pub use tools::{call_tool, tool_definitions};

use std::path::{Path, PathBuf};

use solidcraft_engine::io::vfs::PathScope;

/// The folders `solidcraft-cli mcp` lets its agent read, write and delete in: the system's
/// temporary folders (screenshots), each `allowed` folder (`--allow-path`), and the working
/// folder and the `--in` design's folder when they lie strictly inside the home folder (never
/// home itself, a folder above it or a system folder, where `~/.ssh` or `~/.bashrc` would be
/// in reach). Also the notes to print about folders left out.
pub fn agent_scope(design_dir: Option<PathBuf>, allowed: impl IntoIterator<Item = PathBuf>) -> (PathScope, Vec<String>) {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).filter(|h| !h.is_empty()).map(PathBuf::from);
    let mut roots = vec![std::env::temp_dir()];
    if cfg!(unix) {
        roots.push("/tmp".into());
    }
    let mut notes = Vec::new();
    for (what, dir) in [("working folder", std::env::current_dir().ok()), ("--in design's folder", design_dir)] {
        let Some(dir) = dir else { continue };
        if under_home(&dir, home.as_deref()) {
            roots.push(dir);
        } else {
            notes.push(format!(
                "the {what} {} is not inside your home folder, so it is left out; add it with --allow-path if you mean it",
                dir.display()
            ));
        }
    }
    roots.extend(allowed);
    (PathScope::new(roots), notes)
}

/// Whether `dir` lies strictly inside `home` (both resolved; `false` when either can't be).
pub fn under_home(dir: &Path, home: Option<&Path>) -> bool {
    let (Ok(dir), Some(Ok(home))) = (std::fs::canonicalize(dir), home.map(std::fs::canonicalize)) else { return false };
    dir != home && dir.starts_with(&home)
}

#[cfg(test)]
mod tests;
