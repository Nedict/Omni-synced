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

use std::path::PathBuf;

use solidcraft_engine::io::vfs::PathScope;

/// The folders `solidcraft-cli mcp` lets its agent read, write and delete in: the working folder
/// (unless it is a drive's root, as for a server some clients start in `/`), the system's
/// temporary folders (screenshots) and `extra` (`--in`'s folder, each `--allow-path`).
pub fn agent_scope(extra: impl IntoIterator<Item = PathBuf>) -> PathScope {
    let mut roots: Vec<PathBuf> = std::env::current_dir().into_iter().filter(|d| d.parent().is_some()).collect();
    roots.push(std::env::temp_dir());
    if cfg!(unix) {
        roots.push("/tmp".into());
    }
    roots.extend(extra);
    PathScope::new(roots)
}

#[cfg(test)]
mod tests;
