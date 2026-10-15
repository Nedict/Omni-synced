//! Conversations kept per design: saved at the end of every message, listed in the panel's
//! history menu, readable again and exportable as Markdown. The files and the rules about them
//! (caps, private files, safe names) are `solidcraft_assistant::store`; this turns the panel's
//! items into JSON and back.
//!
//! A saved conversation comes back as a transcript to read. The agent starts fresh: it does not
//! remember it (resuming an agent's own session is `session/load`, a separate piece of work), and
//! the panel says so when one is shown.

use serde_json::{Value, json};
use solidcraft_assistant::store::{self, Saved, Store, Summary};

use super::acp::ToolStatus;
use super::{Item, State};
use crate::SolveApp;
use crate::i18n::tr;

fn root(app: &SolveApp) -> Option<std::path::PathBuf> {
    app.agent_chat.history_root.clone().or_else(|| solidcraft_engine::recovery::data_dir().map(|d| d.join("agent-chat").join("conversations")))
}

fn store(app: &SolveApp) -> Option<Store> {
    root(app).map(Store::at)
}

/// The folder name of the open design's conversations.
fn key(app: &SolveApp) -> String {
    store::key_for(app.session.path.as_deref())
}

fn status_name(s: Option<ToolStatus>) -> &'static str {
    match s {
        Some(ToolStatus::Pending) => "pending",
        Some(ToolStatus::InProgress) => "in_progress",
        Some(ToolStatus::Completed) => "completed",
        Some(ToolStatus::Failed) => "failed",
        None => "",
    }
}

/// An item as JSON; `None` for what is not part of a saved conversation (questions to the user
/// and the design's undo steps are about the moment, not the transcript).
pub fn item_json(item: &Item) -> Option<Value> {
    Some(match item {
        Item::User { text, image } => json!({"kind": "user", "text": text, "image": image}),
        Item::Agent { thought, text } => json!({"kind": "agent", "thought": thought, "text": text}),
        Item::Tool { id, title, kind, status, output } => {
            json!({"kind": "tool", "id": id, "title": title, "tool_kind": kind, "status": status_name(*status), "output": output})
        }
        Item::Plan(entries) => json!({"kind": "plan", "entries": entries.iter().map(|(c, s)| json!([c, s])).collect::<Vec<_>>()}),
        Item::Notice { text, error } => json!({"kind": "notice", "text": text, "error": error}),
        Item::Permission { .. } | Item::Approval { .. } | Item::Changes { .. } => return None,
    })
}

/// An item back from JSON; `None` for anything that is not one.
pub fn item_from_json(v: &Value) -> Option<Item> {
    let text = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
    let flag = |k: &str| v.get(k).and_then(Value::as_bool).unwrap_or(false);
    Some(match v.get("kind")?.as_str()? {
        "user" => Item::User { text: text("text")?, image: flag("image") },
        "agent" => Item::Agent { thought: flag("thought"), text: text("text")? },
        "tool" => Item::Tool {
            id: text("id").unwrap_or_default(),
            title: text("title")?,
            kind: text("tool_kind").unwrap_or_default(),
            status: text("status").and_then(|s| ToolStatus::parse(&s)),
            output: text("output"),
        },
        "plan" => Item::Plan(
            v.get("entries")?
                .as_array()?
                .iter()
                .filter_map(|e| {
                    let p = e.as_array()?;
                    Some((p.first()?.as_str()?.to_string(), p.get(1)?.as_str()?.to_string()))
                })
                .take(100)
                .collect(),
        ),
        "notice" => Item::Notice { text: text("text")?, error: flag("error") },
        _ => return None,
    })
}

/// The conversation's title: the first words the person said.
fn title(items: &[Item]) -> String {
    items
        .iter()
        .find_map(|i| if let Item::User { text, .. } = i { Some(text.lines().next().unwrap_or("").trim().chars().take(60).collect()) } else { None })
        .unwrap_or_default()
}

/// Save the conversation (at the end of a message). Nothing is written with AI features off, and
/// a failure is logged, not shown: the chat works without a history.
pub fn save(app: &mut SolveApp) {
    if !app.preferences.ai_features {
        return;
    }
    let items: Vec<Value> = app.agent_chat.items.iter().filter_map(item_json).collect();
    if !items.iter().any(|i| i["kind"] == "user") {
        return;
    }
    let Some(s) = store(app) else { return };
    let id = app.agent_chat.conv.get_or_insert_with(store::new_id).clone();
    let saved = Saved { id, title: title(&app.agent_chat.items), updated: store::now(), agent: app.agent_chat.agent.clone(), items };
    if let Err(e) = s.save(&key(app), &saved) {
        log::warn!("agent chat history: {e}");
    }
}

/// The open design's saved conversations, newest first.
pub fn list(app: &SolveApp) -> Vec<Summary> {
    if !app.preferences.ai_features {
        return Vec::new();
    }
    store(app).map(|s| s.list(&key(app))).unwrap_or_default()
}

/// Show a saved conversation of the open design (not while a message is being answered).
pub fn load(app: &mut SolveApp, id: &str) -> bool {
    if app.agent_chat.state() == State::Busy {
        return false;
    }
    let Some(saved) = store(app).and_then(|s| s.load(&key(app), id)) else { return false };
    app.agent_chat.items = saved.items.iter().filter_map(item_from_json).collect();
    app.agent_chat.conv = Some(saved.id);
    app.agent_chat.notice(tr("ui.agent_chat_history_loaded"), false);
    true
}

/// Start an empty conversation (the agent keeps running; it just gets a new transcript).
pub fn new_conversation(app: &mut SolveApp) {
    if app.agent_chat.state() == State::Busy {
        return;
    }
    app.agent_chat.items.clear();
    app.agent_chat.conv = None;
}

/// The conversation as Markdown.
pub fn export(app: &SolveApp) -> String {
    let items: Vec<Value> = app.agent_chat.items.iter().filter_map(item_json).collect();
    store::export_markdown(&items)
}
