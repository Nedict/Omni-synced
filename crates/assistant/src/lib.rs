//! The parts of the in-app assistant (the agent chat) that need no window or toolkit: the
//! Markdown an assistant's text is written in, and the plain data and rules around a
//! conversation. The panel in `solidcraft-ui-egui` draws them.
//!
//! Everything here reads text from outside (a model's or an agent's replies, files on disk), so
//! nothing may panic and work and output are bounded.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![forbid(unsafe_code)]

pub mod markdown;
