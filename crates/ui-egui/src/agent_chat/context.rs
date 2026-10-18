//! What the user had selected in the model, as context for a message ("Attach selection", #2286).
//! The selection is described when the message is sent: faces and edges with the names and
//! numbers the agent's tools take (and a size and place that survive an edit), features by name and
//! kind, bodies by name. The conversation shows the same items as chips; the agent gets them as a
//! block of text after the message.

use std::collections::HashMap;

use serde_json::{Value, json};
use solidcraft_engine::{Sel, Session};

/// The most items one message carries; the rest are counted, not dropped silently.
pub const MAX_ITEMS: usize = 64;

/// What an item is, for grouping the chips: a face, edge or vertex; a step of the timeline; a body
/// or a placed component; anything else (planes, axes, sketch geometry).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Element,
    Feature,
    Body,
    Other,
}

impl Kind {
    pub const ALL: [Kind; 4] = [Kind::Element, Kind::Feature, Kind::Body, Kind::Other];

    /// The message id of the group's heading.
    pub fn heading(self) -> &'static str {
        match self {
            Kind::Element => "ui.agent_chat_ctx_elements",
            Kind::Feature => "ui.agent_chat_ctx_features",
            Kind::Body => "ui.agent_chat_ctx_bodies",
            Kind::Other => "ui.agent_chat_ctx_other",
        }
    }

    fn english(self) -> &'static str {
        match self {
            Kind::Element => "Elements",
            Kind::Feature => "Features",
            Kind::Body => "Bodies",
            Kind::Other => "Other",
        }
    }
}

/// One selected thing: its short name for a chip, and what is known about it.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub kind: Kind,
    pub label: String,
    pub detail: String,
}

/// The things a message was sent with.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Context {
    pub items: Vec<Entry>,
    /// Selected things past [`MAX_ITEMS`], not described.
    pub omitted: usize,
}

impl Context {
    pub fn is_empty(&self) -> bool {
        self.items.is_empty() && self.omitted == 0
    }

    /// Everything counted, listed or not.
    pub fn total(&self) -> usize {
        self.items.len() + self.omitted
    }

    /// Add the items of another message (queued messages go as one prompt), up to the limit.
    pub fn merge(&mut self, other: Context) {
        self.omitted += other.omitted;
        for item in other.items {
            if self.items.len() < MAX_ITEMS {
                self.items.push(item);
            } else {
                self.omitted += 1;
            }
        }
    }

    /// The block of text the agent gets after the message, `""` when nothing was selected.
    pub fn prompt_block(&self) -> String {
        if self.is_empty() {
            return String::new();
        }
        let mut out = format!(
            "[Selected in SolidCraft when this message was sent: {} item(s). The names, indices and points are the ones the tools take.]",
            self.total()
        );
        for kind in Kind::ALL {
            let mut lines = self.items.iter().filter(|i| i.kind == kind).peekable();
            if lines.peek().is_none() {
                continue;
            }
            out.push_str(&format!("\n{}:", kind.english()));
            for item in lines {
                out.push_str(&format!("\n- {}", item.label));
                if !item.detail.is_empty() {
                    out.push_str(&format!(": {}", item.detail));
                }
            }
        }
        if self.omitted > 0 {
            out.push_str(&format!("\n(and {} more, not listed)", self.omitted));
        }
        out
    }
}

/// The message as the agent gets it: the text, then what was selected.
pub fn with_context(text: &str, context: &Context) -> String {
    let block = context.prompt_block();
    if block.is_empty() { text.to_string() } else { format!("{text}\n\n{block}") }
}

/// Describe what the session has selected.
pub fn describe_selection(session: &mut Session) -> Context {
    let selection = session.selection.clone();
    describe(session, &selection)
}

/// Describe a selection. Face and edge details come from the model's own listings (`model.faces`,
/// `model.edges`), asked once per body.
pub fn describe(session: &mut Session, selection: &[Sel]) -> Context {
    let mut context = Context::default();
    let mut listings: HashMap<(&'static str, String), Value> = HashMap::new();
    for sel in selection {
        if context.items.len() >= MAX_ITEMS {
            context.omitted += 1;
        } else {
            context.items.push(entry(session, sel, &mut listings));
        }
    }
    context
}

fn entry(session: &mut Session, sel: &Sel, listings: &mut HashMap<(&'static str, String), Value>) -> Entry {
    match sel {
        Sel::Face { body, index, point } => {
            let face = listed(session, listings, "model.faces", "faces", body, *index);
            let mut parts = Vec::new();
            // A name a person can read: the shape and its size, then the body. What the tools take
            // (the persistent name, the index, the pick point) is in the detail.
            let mut shape = format!("Face #{index}");
            if let Some(f) = &face {
                let radius = f["cylinder"]["radius"].as_f64();
                let planar = f["plane_normal"].is_array();
                let area = f["area"].as_f64().map(|a| format!(" {} mm²", num(a))).unwrap_or_default();
                shape = match (radius, planar) {
                    (Some(r), _) => format!("Round face r{}", num(r)),
                    (None, true) => format!("Flat face{area}"),
                    _ => format!("Curved face{area}"),
                };
                if let Some(name) = f["name"].as_str() {
                    parts.push(format!("name {name}"));
                }
                parts.push(match (radius, planar) {
                    (Some(r), _) => format!("cylindrical, radius {} mm", num(r)),
                    (None, true) => format!("planar, normal {}", point3(&f["plane_normal"])),
                    _ => "curved".to_string(),
                });
                if let Some(a) = f["area"].as_f64() {
                    parts.push(format!("area {} mm²", num(a)));
                }
                parts.push(format!("centroid {}", point3(&f["centroid"])));
                parts.push(format!("index {index}"));
            }
            parts.push(format!("pick point {}", point3(&json!(point))));
            Entry { kind: Kind::Element, label: format!("{shape} · {body}"), detail: parts.join(", ") }
        }
        Sel::Edge { body, index, point } => {
            let edge = listed(session, listings, "model.edges", "edges", body, *index);
            let mut parts = Vec::new();
            let mut shape = format!("Edge #{index}");
            if let Some(e) = &edge {
                if let Some(name) = e["name"].as_str() {
                    parts.push(format!("name {name}"));
                }
                if let Some(l) = e["length"].as_f64() {
                    shape = format!("Edge {} mm", num(l));
                    parts.push(format!("length {} mm", num(l)));
                }
                parts.push(format!("from {} to {}", point3(&e["start"]), point3(&e["end"])));
                parts.push(format!("index {index}"));
            }
            parts.push(format!("pick point {}", point3(&json!(point))));
            Entry { kind: Kind::Element, label: format!("{shape} · {body}"), detail: parts.join(", ") }
        }
        Sel::Vertex { body, point } => {
            Entry { kind: Kind::Element, label: format!("Vertex of {body}"), detail: format!("at {}", point3(&json!(point))) }
        }
        Sel::Body { name } => Entry { kind: Kind::Body, label: name.clone(), detail: String::new() },
        Sel::Occurrence { path } => Entry {
            kind: Kind::Body,
            label: format!("Component occurrence {}", path.iter().map(u64::to_string).collect::<Vec<_>>().join("/")),
            detail: format!("occurrence path {path:?}"),
        },
        Sel::Feature { id } => {
            let feature = session.doc.find_feature(&id.to_string());
            let name = feature.map(|f| f.name.clone()).unwrap_or_else(|| format!("#{id}"));
            let kind = feature.and_then(|f| serde_json::to_value(f).ok()).and_then(|v| v["type"].as_str().map(str::to_string));
            let mut detail = format!("feature id {id}");
            if let Some(kind) = kind {
                detail = format!("{kind}, {detail}");
            }
            if feature.is_some_and(|f| f.suppressed) {
                detail.push_str(", suppressed");
            }
            Entry { kind: Kind::Feature, label: name, detail }
        }
        other => {
            // Planes, axes and sketch geometry: named by their kind and whatever identifies them.
            let value = serde_json::to_value(other).unwrap_or(Value::Null);
            let kind = value["type"].as_str().unwrap_or("item").replace('_', " ");
            let id = ["name", "id", "index", "sketch"].iter().find_map(|k| match &value[k] {
                Value::String(s) => Some(s.clone()),
                Value::Number(n) => Some(n.to_string()),
                _ => None,
            });
            let label = match id {
                Some(id) => format!("{kind} {id}"),
                None => kind,
            };
            let label: String = label.chars().take(60).collect();
            Entry { kind: Kind::Other, label: capitalised(&label), detail: String::new() }
        }
    }
}

/// The listing entry for a face or edge (`index`), from a command run once per body.
fn listed(
    session: &mut Session,
    listings: &mut HashMap<(&'static str, String), Value>,
    command: &'static str,
    list: &str,
    body: &str,
    index: usize,
) -> Option<Value> {
    let listing =
        listings.entry((command, body.to_string())).or_insert_with(|| session.execute(command, &json!({"body": body})).unwrap_or(Value::Null));
    let index = u64::try_from(index).ok()?;
    listing[list].as_array()?.iter().find(|item| item["index"].as_u64() == Some(index)).cloned()
}

/// A length or area to two decimals, without a trailing `.00`.
fn num(v: f64) -> String {
    if !v.is_finite() {
        return "?".into();
    }
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// `(x, y, z)` from a JSON array of three numbers, `?` when it is not one.
fn point3(v: &Value) -> String {
    match v.as_array().map(|a| a.iter().filter_map(Value::as_f64).collect::<Vec<_>>()) {
        Some(c) if c.len() == 3 => format!("({}, {}, {})", num(c[0]), num(c[1]), num(c[2])),
        _ => "?".into(),
    }
}

fn capitalised(s: &str) -> String {
    let mut chars = s.chars();
    chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
}
