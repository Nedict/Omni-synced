//! Editable, illustrative material-only estimates. Prices are EUR, volumes are m³.
//!
//! The purchase reference is a user-provided panel, not a market quotation. Bounding-box
//! stock is measured in each body's definition frame; assembly rotations never enlarge it.
//! Quantities count every component placement, including repeated nested instances.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::CommandSpec;
use crate::params::bad;
use crate::{Result, Session};

/// Namespaced design metadata; unrelated metadata remains untouched.
pub const METADATA_KEY: &str = "org.solidcraft.material_estimate";
const ESTIMATE: &str = "material.estimate";
const SETTINGS: &str = "material.estimate.settings";
const MAX_DIMENSION_MM: f64 = 10_000_000.0;
const MAX_VOLUME_M3: f64 = 1e12;
const MAX_COST: f64 = 1e18;
const MAX_BODIES: u64 = 1_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Basis {
    /// Finished solid volume, after cuts and recorded sheet perforations.
    NetVolume,
    /// One rectangular stock blank per body, in that body's definition frame.
    BoundingBox,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub reference_name: String,
    pub reference_price: f64,
    pub reference_length_mm: f64,
    pub reference_width_mm: f64,
    pub reference_thickness_mm: f64,
    pub waste_percent: f64,
    pub basis: Basis,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            reference_name: "Oak — example".into(),
            reference_price: 180.0,
            reference_length_mm: 1400.0,
            reference_width_mm: 750.0,
            reference_thickness_mm: 40.0,
            waste_percent: 15.0,
            basis: Basis::BoundingBox,
        }
    }
}

impl Settings {
    fn validate(&self, cmd: &str) -> Result<f64> {
        let name = self.reference_name.trim();
        if name.is_empty() || name.chars().count() > 128 || name.chars().any(char::is_control) {
            return Err(bad(cmd, "reference_name must contain 1–128 characters without control characters"));
        }
        if !self.reference_price.is_finite() || !(0.0..=1e12).contains(&self.reference_price) {
            return Err(bad(cmd, "reference_price must be a finite EUR amount between 0 and 1000000000000"));
        }
        let dimensions = [self.reference_length_mm, self.reference_width_mm, self.reference_thickness_mm];
        if dimensions.iter().any(|d| !d.is_finite() || !(0.001..=MAX_DIMENSION_MM).contains(d)) {
            return Err(bad(cmd, "reference dimensions must be finite millimetres between 0.001 and 10000000"));
        }
        if !self.waste_percent.is_finite() || !(0.0..=1000.0).contains(&self.waste_percent) {
            return Err(bad(cmd, "waste_percent must be finite and between 0 and 1000"));
        }
        let volume = dimensions.into_iter().map(|d| d / 1000.0).product::<f64>();
        let price_per_m3 = self.reference_price / volume;
        if !volume.is_finite() || volume <= 0.0 || !price_per_m3.is_finite() || price_per_m3 > 1e15 {
            return Err(bad(cmd, "reference price per m³ exceeds the supported numeric range"));
        }
        Ok(price_per_m3)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Row {
    pub name: String,
    pub material: String,
    pub quantity: u64,
    /// Dimensions of one body; the volumes and cost below include every placement.
    pub dimensions_mm: [f64; 3],
    pub net_volume_m3: f64,
    pub stock_volume_m3: f64,
    pub estimated_volume_m3: f64,
    pub cost: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Totals {
    pub body_count: u64,
    pub net_volume_m3: f64,
    pub stock_volume_m3: f64,
    pub estimated_volume_m3: f64,
    pub cost: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub settings: Settings,
    pub reference_price_per_m3: f64,
    pub bodies: Vec<Row>,
    pub total: Totals,
}

pub static COMMANDS: &[CommandSpec] = &[
    CommandSpec::new(ESTIMATE, "Estimate Materials", estimate)
        .at("SOLID", "INSPECT")
        .icon("material")
        .noundo()
        .doc_only()
        .params("settings?: full reference settings override (read-only). EUR material-only estimate of all placed B-rep solids; definition-frame stock boxes or net volume, plus waste. Default illustrative oak reference: 180 EUR for 1400 × 750 × 40 mm; not a market price. Meshes, open surfaces, stale or failed models are refused"),
    CommandSpec::new(SETTINGS, "Material Estimate Reference", save_settings)
        .icon("material")
        .doc_only()
        .params("settings: {reference_name, reference_price (EUR), reference_length_mm, reference_width_mm, reference_thickness_mm, waste_percent, basis: net_volume|bounding_box}. Saves the validated editable reference in design metadata, in one undo step"),
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EstimateParams {
    settings: Option<Settings>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SaveParams {
    settings: Settings,
}

fn decode<T: serde::de::DeserializeOwned>(value: &Value, cmd: &str) -> Result<T> {
    serde_json::from_value(value.clone()).map_err(|error| bad(cmd, error.to_string()))
}

/// Read saved settings without silently substituting defaults for damaged metadata.
pub fn settings(s: &Session) -> Result<Settings> {
    let settings: Settings = match s.doc.metadata.get(METADATA_KEY) {
        Some(value) => decode(value, ESTIMATE)?,
        None => Settings::default(),
    };
    settings.validate(ESTIMATE)?;
    Ok(settings)
}

fn save_settings(s: &mut Session, params: &Value) -> Result<Value> {
    let params: SaveParams = decode(params, SETTINGS)?;
    let price_per_m3 = params.settings.validate(SETTINGS)?;
    let value = serde_json::to_value(&params.settings).map_err(|error| bad(SETTINGS, error.to_string()))?;
    let mut metadata = s.doc.metadata.clone();
    let changed = metadata.set(METADATA_KEY, value).map_err(|error| bad(SETTINGS, error.to_string()))?;
    // Serialization and all validation finish before touching the document.
    let response = json!({"settings": params.settings, "reference_price_per_m3": price_per_m3, "changed": changed});
    if changed {
        s.doc_mut().metadata = metadata;
    }
    Ok(response)
}

fn supported(value: f64, max: f64, what: &str) -> Result<f64> {
    if value.is_finite() && (0.0..=max).contains(&value) {
        Ok(value)
    } else {
        Err(bad(ESTIMATE, format!("{what} exceeds the supported numeric range")))
    }
}

fn estimate(s: &mut Session, params: &Value) -> Result<Value> {
    if params.get("settings").is_some_and(Value::is_null) {
        return Err(bad(ESTIMATE, "settings must be a complete reference settings object"));
    }
    let params: EstimateParams = decode(params, ESTIMATE)?;
    let settings = match params.settings {
        Some(settings) => settings,
        None => settings(s)?,
    };
    let reference_price_per_m3 = settings.validate(ESTIMATE)?;
    if s.model.is_stale() {
        return Err(bad(ESTIMATE, "finish the active gesture before estimating materials; the model is stale"));
    }
    if !s.model.param_errors.is_empty() || s.model.results.iter().any(|result| !result.skipped && result.error.is_some()) {
        return Err(bad(ESTIMATE, "resolve failed features before estimating materials"));
    }
    let placements = super::component::properties::placements(&s.doc, ESTIMATE)?;
    let mut quantities = BTreeMap::<u64, u64>::new();
    for (component, _, _) in placements {
        let count = quantities.entry(component).or_default();
        *count = count.checked_add(1).ok_or_else(|| bad(ESTIMATE, "too many component placements"))?;
    }
    let state = s.model.state();
    let mut bodies = Vec::new();
    let mut total = Totals::default();
    for body in &state.bodies {
        let quantity = quantities.get(&s.doc.model_body_component(body)).copied().unwrap_or(0);
        if quantity == 0 {
            continue;
        }
        total.body_count = total
            .body_count
            .checked_add(quantity)
            .filter(|n| *n <= MAX_BODIES)
            .ok_or_else(|| bad(ESTIMATE, "estimate exceeds the 1000000 placed-body limit"))?;
        if body.body.is_mesh() || body.body.is_surface() {
            return Err(bad(ESTIMATE, format!("`{}` is not a closed B-rep solid; mesh and surface estimates are not supported yet", body.name)));
        }
        let measured = solidcraft_kernel::measure(&body.body)?;
        let extent = measured.bbox.max - measured.bbox.min;
        let dimensions_mm = [extent.x, extent.y, extent.z];
        if !measured.bbox.min.is_finite()
            || !measured.bbox.max.is_finite()
            || dimensions_mm.iter().any(|dimension| !dimension.is_finite() || *dimension <= 0.0 || *dimension > MAX_DIMENSION_MM)
        {
            return Err(bad(ESTIMATE, format!("`{}` has unsupported stock dimensions", body.name)));
        }
        let mut net_mm3 = measured.volume;
        // Perforation fields are recorded on sheets without cutting each hole into the B-rep.
        if let Some(sheet) = state.sheets.iter().find(|sheet| sheet.body == body.name) {
            let removed = supported(sheet.perforated_area() * sheet.t, MAX_VOLUME_M3 * 1e9, "sheet perforation volume")?;
            net_mm3 -= removed;
        }
        if !net_mm3.is_finite() || net_mm3 <= 0.0 {
            return Err(bad(ESTIMATE, format!("`{}` has no valid positive solid volume", body.name)));
        }
        let quantity_f = quantity as f64;
        let net_volume_m3 = supported(net_mm3 / 1e9 * quantity_f, MAX_VOLUME_M3, "net volume")?;
        let stock_volume_m3 = supported(dimensions_mm.into_iter().map(|d| d / 1000.0).product::<f64>() * quantity_f, MAX_VOLUME_M3, "stock volume")?;
        let base = match settings.basis {
            Basis::NetVolume => net_volume_m3,
            Basis::BoundingBox => stock_volume_m3,
        };
        let estimated_volume_m3 = supported(base * (1.0 + settings.waste_percent / 100.0), MAX_VOLUME_M3, "estimated volume")?;
        let cost = supported(estimated_volume_m3 * reference_price_per_m3, MAX_COST, "estimated cost")?;
        total.net_volume_m3 = supported(total.net_volume_m3 + net_volume_m3, MAX_VOLUME_M3, "total net volume")?;
        total.stock_volume_m3 = supported(total.stock_volume_m3 + stock_volume_m3, MAX_VOLUME_M3, "total stock volume")?;
        total.estimated_volume_m3 = supported(total.estimated_volume_m3 + estimated_volume_m3, MAX_VOLUME_M3, "total estimated volume")?;
        total.cost = supported(total.cost + cost, MAX_COST, "total cost")?;
        bodies.push(Row {
            name: body.name.clone(),
            material: s.doc.materials.get(body.definition_name()).cloned().unwrap_or_else(|| "Default".into()),
            quantity,
            dimensions_mm,
            net_volume_m3,
            stock_volume_m3,
            estimated_volume_m3,
            cost,
        });
    }
    let report = Report { settings, reference_price_per_m3, bodies, total };
    serde_json::to_value(report).map_err(|error| bad(ESTIMATE, error.to_string()))
}

#[cfg(test)]
#[path = "material_estimate_tests.rs"]
mod tests;
