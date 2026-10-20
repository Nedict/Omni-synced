//! Tokenised dense card library. Import elements through `crate::ui_kit`.
//! `Panel` owns card composition; rows own aligned label/control columns and retain egui focus.
//! Geometry, typography and colours come from `theme::Tokens`. See `docs/ui-kit.md`.
pub mod actions;
pub mod choice;
pub mod fields;
pub mod layout;
pub mod metrics;
pub(crate) mod panel;
pub mod status;
pub mod table;
pub mod text;

pub use actions::*;
pub use choice::*;
pub use fields::*;
pub use layout::*;
pub use metrics::{header_height, row_height};
pub(crate) use panel::{ComputeStatus, FeatureOptions, Panel, Part, Prompt};
pub use panel::{collapsible_header, header};
pub use status::*;
pub use table::*;

#[cfg(test)]
mod gallery;
#[cfg(test)]
#[path = "test_support_tests.rs"]
mod test_support;
