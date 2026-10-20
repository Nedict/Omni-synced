//! Tokenised dense card library. Import elements through `crate::ui_kit`.
//! `Panel` owns card composition; rows own aligned label/control columns and retain egui focus.
//! Geometry, typography and colours come from `theme::Tokens`. See `docs/ui-kit.md`.
mod actions;
mod checks;
mod choice;
mod color;
mod disclosure;
mod fields;
mod layout;
mod metrics;
pub(crate) mod panel;
mod reports;
mod status;
mod table;
mod text;

pub(crate) use actions::*;
pub(crate) use checks::*;
pub(crate) use choice::*;
pub(crate) use color::*;
pub(crate) use disclosure::*;
pub(crate) use fields::*;
pub(crate) use layout::*;
pub(crate) use metrics::{ContentWidth, header_height, row_height, table_row_height};
pub(crate) use panel::{ComputeStatus, FeatureOptions, Panel, Part, Prompt};
#[cfg(test)]
pub(crate) use panel::{collapsible_header, header};
pub(crate) use reports::*;
pub(crate) use status::*;
pub(crate) use table::*;

#[cfg(test)]
mod gallery;
#[cfg(test)]
mod slider_tests;
#[cfg(test)]
#[path = "test_support_tests.rs"]
mod test_support;

pub(crate) use layout::label as row_label;
