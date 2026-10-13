//! Tool windows for sketch-space modify commands, on the `dialogs_surface` pattern (#19): one
//! `Mod` enum behind one `Kind::SketchModify` variant, grown by one variant and one arm per
//! command rather than by a new module each time. The first variant here is Scale, whose
//! toolbar button reached no tool and no dialog at all, so a press ran `sketch.scale` with no
//! parameters and the engine answered "`factor` must be a positive number".
//! `sketch.offset`, `sketch.fillet`, the three `sketch.chamfer.*` commands, the sketch
//! patterns, `sketch.mirror` and `sketch.move` are the same gap and join this module later,
//! each its own change.

use serde_json::{Map, Value, json};
use solidcraft_engine::Sel;
use solidcraft_engine::doc::expr::Kind as ValueKind;

use crate::SolveApp;
use crate::dialogs::{Kind, field, row_label};
use crate::selection::{CURVES, POINTS, SelInput};

/// A sketch modify tool window's values.
#[derive(Clone, Debug, PartialEq)]
pub enum Mod {
    /// `sketch.scale`: the factor the picked entities scale by. The base point is the second
    /// input; with none picked the command scales about the sketch origin, as its own default
    /// already does.
    Scale { factor: String },
}

impl Mod {
    pub fn title(&self) -> &'static str {
        match self {
            Mod::Scale { .. } => "SKETCH SCALE",
        }
    }

    /// The value on the canvas.
    pub fn primary(&mut self) -> Option<(&'static str, ValueKind, &mut String)> {
        Some(match self {
            Mod::Scale { factor } => ("Factor", ValueKind::Unitless, factor),
        })
    }
}

fn inputs(k: &Mod) -> Vec<SelInput> {
    match k {
        Mod::Scale { .. } => vec![SelInput::new("Entities", CURVES | POINTS, true), SelInput::new("Base point", POINTS, false)],
    }
}

/// The dialog for a sketch modify command.
pub fn start(_app: &SolveApp, id: &str) -> Option<(Kind, Vec<SelInput>)> {
    let k = match id {
        "sketch.scale" => Mod::Scale { factor: "2".into() },
        _ => return None,
    };
    let i = inputs(&k);
    Some((Kind::SketchModify(k), i))
}

/// The rows after the inputs; true when Enter was pressed in a field.
pub fn rows(ui: &mut egui::Ui, k: &mut Mod) -> bool {
    let mut enter = false;
    match k {
        Mod::Scale { factor } => {
            row_label(ui, "Factor");
            enter |= field(ui, factor);
            ui.end_row();
        }
    }
    enter
}

/// What input `i` holds.
fn items(inputs: &[SelInput], i: usize) -> &[Sel] {
    inputs.get(i).map(|x| x.items.as_slice()).unwrap_or(&[])
}

/// A picked sketch point as the id its own sketch knows it by. Picking prefixes the sketch
/// ("`<sketch>:<point>`") so points of any visible sketch stay distinct; a sketch command
/// names the point alone.
fn point_id(sel: &Sel) -> Option<&str> {
    match sel {
        Sel::SketchPoint { id } => Some(id.split_once(':').map(|(_, p)| p).unwrap_or(id)),
        _ => None,
    }
}

/// The curves and lone points picked in input `i`, as the engine's `entities` list.
fn entity_ids(inputs: &[SelInput], i: usize) -> Vec<String> {
    items(inputs, i)
        .iter()
        .filter_map(|x| match x {
            Sel::SketchCurve { id } => Some(id.clone()),
            _ => point_id(x).map(str::to_string),
        })
        .collect()
}

/// A typed scale factor as the command wants it: the field evaluated against the document's
/// parameters, in the range the command accepts. Checking the range here lets the window say
/// so in its own error line instead of the status bar.
fn factor(app: &SolveApp, text: &str) -> Result<f64, String> {
    let v = app.session.doc.eval(text.trim(), ValueKind::Unitless).map_err(|e| format!("Factor: {e}"))?;
    if !(v.is_finite() && v > 1e-9 && v < 1e6) {
        return Err("Factor must be a positive number".into());
    }
    Ok(v)
}

/// The command a sketch modify dialog makes. `sketch.scale` takes a plain evaluated number for
/// its factor, not an expression string: it has no model parameter to carry one, unlike
/// `sketch.fillet`'s radius.
pub fn commands(app: &SolveApp, k: &Mod, inputs: &[SelInput], extra: &Map<String, Value>) -> Result<Vec<(String, Value)>, String> {
    let (cmd, mut params) = match k {
        Mod::Scale { factor: text } => {
            let entities = entity_ids(inputs, 0);
            if entities.is_empty() {
                return Err("Select the sketch curves or points to scale first".into());
            }
            let f = factor(app, text)?;
            let mut p = json!({"entities": entities, "factor": f});
            if let Some(base) = items(inputs, 1).first().and_then(point_id) {
                p["base"] = json!(base);
            }
            ("sketch.scale", p)
        }
    };
    if let Value::Object(m) = &mut params {
        for (key, value) in extra {
            m.entry(key.clone()).or_insert_with(|| value.clone());
        }
    }
    Ok(vec![(cmd.to_string(), params)])
}
