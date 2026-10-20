//! `solvecraft-cli`: legacy compatibility alias, retained so older scripts and MCP client
//! setups continue to work. It runs `omni-synced-cli` next to the binary, or on PATH, with the
//! same arguments and streams, then exits with the same status after a brief deprecation note.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![forbid(unsafe_code)]

use std::process::{Command, ExitCode};

const TARGET: &str = "omni-synced-cli";

fn main() -> ExitCode {
    eprintln!("solvecraft-cli is deprecated and will be removed in a later release: use omni-synced-cli");
    let sibling = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(format!("{TARGET}{}", std::env::consts::EXE_SUFFIX))))
        .filter(|p| p.is_file());
    let program = sibling.map(|p| p.into_os_string()).unwrap_or_else(|| TARGET.into());
    match Command::new(&program).args(std::env::args_os().skip(1)).status() {
        Ok(status) => match status.code() {
            Some(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
            None => ExitCode::FAILURE,
        },
        Err(e) => {
            eprintln!("solvecraft-cli: could not run {}: {e}", program.to_string_lossy());
            ExitCode::FAILURE
        }
    }
}
