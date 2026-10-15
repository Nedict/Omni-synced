//! Agent chat: a side panel that talks to a coding agent the user already has (Claude Code,
//! Codex, Gemini CLI, OpenCode, …) over the Agent Client Protocol, and gives that agent
//! SolidCraft's MCP server, bridged to this window, as its way to read and change the open design
//! (docs/agent-chat.md, docs/design/in-app-agent-chat.md).
//!
//! Desktop only: the agent is a child process. Off unless turned on in Preferences. SolidCraft
//! never handles the agent's credentials: an agent that needs a login says so, and the panel
//! shows the agent's own login command for the user to run.
//!
//! - `backend.rs`: the [`backend::ChatBackend`] seam; `acp_client.rs` is the ACP backend.
//! - `gate.rs`: what the agent's tool requests may do (run, ask the user first, or never).
//! - `panel.rs`: the panel itself (docking, minimising, the picker, the conversation).
//! - Here: the conversation, turns (each one undoes as one step), and the requests waiting for
//!   Allow or Deny.

// The web build shows only a note: the process side is desktop only.
#![cfg_attr(target_arch = "wasm32", allow(dead_code, unused_imports))]

pub mod acp;
#[cfg(not(target_arch = "wasm32"))]
mod acp_client;
pub mod backend;
#[cfg(not(target_arch = "wasm32"))]
pub mod discover;
pub mod gate;
mod panel;
#[cfg(not(target_arch = "wasm32"))]
mod process;

use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};

use serde_json::{Value, json};

use crate::i18n::{format as trf, tr};
use crate::{ControlRequest, SolveApp};
use backend::{ChatBackend, Event, PermissionOption, PromptPart, ToolStatus};

pub use panel::{show, tip as panel_tip, toggle};

/// Most entries kept in the conversation; older ones are dropped.
const MAX_ITEMS: usize = 2000;
/// Longest message sent.
const MAX_PROMPT: usize = 100_000;
/// How long a request waits for Allow or Deny before it is declined (the agent's MCP server gives
/// up on a request after five minutes).
const APPROVAL_TIMEOUT_MS: f64 = 240_000.0;
/// Size of the picture of the view sent with a message.
const VIEW_IMAGE: (usize, usize) = (960, 600);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Off,
    Starting,
    Ready,
    Busy,
    NeedsAuth,
    Exited,
}

impl State {
    fn id(self) -> &'static str {
        match self {
            State::Off => "off",
            State::Starting => "starting",
            State::Ready => "ready",
            State::Busy => "busy",
            State::NeedsAuth => "auth",
            State::Exited => "exited",
        }
    }
    fn label(self) -> &'static str {
        tr(match self {
            State::Off => "ui.agent_chat_state_off",
            State::Starting => "ui.agent_chat_state_starting",
            State::Ready => "ui.agent_chat_state_ready",
            State::Busy => "ui.agent_chat_state_busy",
            State::NeedsAuth => "ui.agent_chat_state_auth",
            State::Exited => "ui.agent_chat_state_exited",
        })
    }
    fn running(self) -> bool {
        matches!(self, State::Starting | State::Ready | State::Busy | State::NeedsAuth)
    }
}

#[derive(Clone, Debug)]
pub enum Item {
    User { text: String, image: bool },
    Agent { thought: bool, text: String },
    Tool { id: String, title: String, kind: String, status: Option<ToolStatus>, output: Option<String> },
    Plan(Vec<(String, String)>),
    /// The agent asks before running something of its own (its options, its words).
    Permission { key: u64, title: String, options: Vec<PermissionOption>, answer: Option<String> },
    /// A request of the agent's that SolidCraft holds for the user (files, replacing the design).
    Approval { id: u64, title: String, answer: Option<bool> },
    /// A message's changes, merged into one undo step; `doc`: the design right after it.
    Changes { steps: usize, doc: Arc<solidcraft_engine::doc::Document>, undone: bool },
    Notice { text: String, error: bool },
}

/// The message being answered: what undo and the limits need.
struct Turn {
    /// The design and the undo depth before it.
    start_doc: Arc<solidcraft_engine::doc::Document>,
    start_undo: usize,
    /// Requests that ran commands, and requests of any kind.
    changes: u32,
    requests: u32,
    limit_told: bool,
    label: String,
}

/// A request held for Allow or Deny.
struct Held {
    id: u64,
    req: ControlRequest,
    since: f64,
}

/// Starts the chat's own control endpoint for the agent's MCP server (the desktop host provides
/// it): requests arriving there go to the sender; the port it listens on.
pub type ControlStarter = Box<dyn FnMut(Sender<ControlRequest>) -> Option<u16>>;

#[derive(Default)]
pub struct AgentChat {
    /// The panel is shown (View menu, the application bar's chat button, Ctrl+Shift+A).
    pub open: bool,
    pub items: Vec<Item>,
    pub input: String,
    state: Option<State>,
    agent: String,
    images: bool,
    /// Send a picture of the view with the next message.
    pub attach_view: bool,
    auth: Vec<acp::AuthMethod>,
    auth_status: Option<String>,
    /// A message typed before the agent was ready.
    queued: Option<(String, bool)>,
    /// The last message sent, kept to send again after a sign-in.
    last: Option<(String, bool)>,
    /// The command the running agent was started with.
    argv: Vec<String>,
    backend: Option<Box<dyn ChatBackend>>,
    turn: Option<Turn>,
    held: Vec<Held>,
    next_approval: u64,
    /// The chat's control endpoint: the port, and its requests.
    pub tools_port: Option<u16>,
    tools_rx: Option<Receiver<ControlRequest>>,
    tools_tx: Option<Sender<ControlRequest>>,
    /// Desktop host hook: start the endpoint on demand.
    pub control_starter: Option<ControlStarter>,
    custom: bool,
    custom_text: String,
    /// The picker is unfolded (it folds to one line once an agent is connected).
    picking: Option<bool>,
    #[cfg(not(target_arch = "wasm32"))]
    detection: Option<discover::Detection>,
    #[cfg(not(target_arch = "wasm32"))]
    detecting: Option<Receiver<discover::Detection>>,
}

impl Drop for AgentChat {
    fn drop(&mut self) {
        self.stop_backend(true);
    }
}

impl AgentChat {
    pub fn state(&self) -> State {
        self.state.unwrap_or(State::Off)
    }
    fn push(&mut self, item: Item) {
        if self.items.len() >= MAX_ITEMS {
            self.items.drain(..MAX_ITEMS / 10);
        }
        self.items.push(item);
    }
    fn notice(&mut self, text: impl Into<String>, error: bool) {
        self.push(Item::Notice { text: text.into(), error });
    }
    /// Requests waiting for Allow or Deny.
    pub fn waiting(&self) -> usize {
        self.held.len() + self.items.iter().filter(|i| matches!(i, Item::Permission { answer: None, .. })).count()
    }

    fn stop_backend(&mut self, wait: bool) {
        if let Some(mut b) = self.backend.take() {
            b.cancel();
            b.shutdown(wait);
        }
        for h in self.held.drain(..) {
            let _ = h.req.reply.send(json!({"ok": false, "error": "the agent chat stopped"}));
        }
        for i in &mut self.items {
            if let Item::Approval { answer: a @ None, .. } = i {
                *a = Some(false);
            }
            if let Item::Permission { answer: a @ None, .. } = i {
                *a = Some(tr("ui.agent_chat_cancel").to_string());
            }
        }
    }

    /// Answer the agent's own permission question at `index` (`None`: cancelled).
    pub fn answer(&mut self, index: usize, option: Option<&str>) {
        let Some(Item::Permission { key, options, answer, .. }) = self.items.get_mut(index) else { return };
        if answer.is_some() {
            return;
        }
        let label = option.and_then(|o| options.iter().find(|x| x.id == o)).map_or_else(|| tr("ui.agent_chat_cancel").to_string(), |o| o.name.clone());
        *answer = Some(label);
        let key = *key;
        if let Some(b) = self.backend.as_mut() {
            b.answer_permission(key, option);
        }
    }

    /// Ask the agent to sign in by itself (an "agent" login method: it opens the browser).
    pub fn authenticate(&mut self, method: &str) {
        if let Some(b) = self.backend.as_mut() {
            b.authenticate(method);
            self.state = Some(State::Starting);
        }
    }

    /// Take in what the backend reported.
    fn on_event(&mut self, e: Event) -> Option<TurnEnd> {
        match e {
            Event::Ready { agent, images } => {
                self.agent = agent;
                self.images = images;
                self.state = Some(State::Ready);
                self.picking = Some(false);
            }
            Event::Text { thought, text } => {
                if let Some(Item::Agent { thought: t, text: last }) = self.items.last_mut()
                    && *t == thought
                {
                    last.push_str(&text);
                } else {
                    self.push(Item::Agent { thought, text });
                }
            }
            Event::Tool { id, title, kind, status, output } => {
                let existing = self.items.iter_mut().rev().find(|i| matches!(i, Item::Tool { id: x, .. } if *x == id));
                if let Some(Item::Tool { title: t, kind: k, status: s, output: o, .. }) = existing {
                    if let Some(v) = title {
                        *t = v;
                    }
                    if let Some(v) = kind {
                        *k = v;
                    }
                    if status.is_some() {
                        *s = status;
                    }
                    if output.is_some() {
                        *o = output;
                    }
                } else {
                    self.push(Item::Tool { id, title: title.unwrap_or_default(), kind: kind.unwrap_or_default(), status, output });
                }
            }
            Event::Plan(entries) => {
                // A plan is replaced whole: update this turn's plan where it is.
                let turn = self.items.iter().rposition(|i| matches!(i, Item::User { .. })).unwrap_or(0);
                if let Some(Item::Plan(p)) = self.items.iter_mut().skip(turn).rev().find(|i| matches!(i, Item::Plan(_))) {
                    *p = entries;
                } else {
                    self.push(Item::Plan(entries));
                }
            }
            Event::Permission { key, title, options } => self.push(Item::Permission { key, title, options, answer: None }),
            Event::AuthRequired { methods } => {
                self.auth = methods;
                self.state = Some(State::NeedsAuth);
                self.picking = Some(false);
                // What was asked goes again once the login is done.
                if self.queued.is_none() {
                    self.queued = self.last.take();
                }
            }
            Event::AuthStatus(s) => self.auth_status = Some(s),
            Event::TurnEnded { reason } => {
                if self.state() == State::Busy {
                    self.state = Some(State::Ready);
                }
                if !matches!(reason.as_str(), "" | "auth" | "error") {
                    self.notice(format!("({reason})"), false);
                }
                return Some(TurnEnd);
            }
            Event::Error(e) => {
                self.notice(e, true);
                if !self.state().running() || self.state() == State::Starting {
                    self.state = Some(State::Exited);
                }
            }
            Event::Exited { tail } => {
                self.state = Some(State::Exited);
                let last: Vec<String> = tail.into_iter().rev().take(6).collect::<Vec<_>>().into_iter().rev().collect();
                let detail = if last.is_empty() { String::new() } else { format!("\n{}", last.join("\n")) };
                self.notice(format!("{}{detail}", tr("ui.agent_chat_exited")), true);
                self.stop_backend(false);
                return Some(TurnEnd);
            }
        }
        None
    }
}

/// A message was answered (or the agent stopped): the turn's bookkeeping ends.
struct TurnEnd;

/// The command the chat starts: the preference, else the first agent found.
#[cfg(not(target_arch = "wasm32"))]
fn chosen_command(app: &SolveApp) -> Vec<String> {
    let pref = acp::split_command(&app.preferences.agent_command);
    if !pref.is_empty() {
        return pref;
    }
    app.agent_chat.detection.as_ref().and_then(|d| d.found.first()).map(|f| f.argv.clone()).unwrap_or_default()
}

/// `solidcraft-cli` next to this program, else on the search path.
#[cfg(not(target_arch = "wasm32"))]
fn find_cli() -> Option<std::path::PathBuf> {
    let name = if cfg!(windows) { "solidcraft-cli.exe" } else { "solidcraft-cli" };
    if let Some(p) = solidcraft_engine::geom::env::var("CLI").map(std::path::PathBuf::from).filter(|p| p.is_file()) {
        return Some(p);
    }
    std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|d| d.join(name)))
        .filter(|p| p.is_file())
        .or_else(|| discover::find_in("solidcraft-cli", &discover::search_dirs()))
}

/// The agent's working folder: an empty folder of SolidCraft's own, so an agent's own file tools
/// (where it has them) see nothing of the user's.
fn working_dir() -> std::path::PathBuf {
    let d = solidcraft_engine::recovery::data_dir().map(|d| d.join("agent-chat").join("work")).unwrap_or_else(|| std::env::temp_dir().join("solidcraft-agent-chat"));
    #[cfg(not(target_arch = "wasm32"))]
    let _ = std::fs::create_dir_all(&d);
    d
}

/// Start the agent: the chat's control endpoint for its MCP bridge if needed, the process.
#[cfg(not(target_arch = "wasm32"))]
pub fn connect(app: &mut SolveApp, ctx: &egui::Context) {
    disconnect(app);
    let argv = chosen_command(app);
    if argv.is_empty() {
        app.agent_chat.notice(tr("ui.agent_chat_none_found"), true);
        return;
    }
    let chat = &mut app.agent_chat;
    if chat.tools_port.is_none()
        && let Some(start) = chat.control_starter.as_mut()
    {
        let (tx, rx) = std::sync::mpsc::channel();
        if let Some(port) = start(tx.clone()) {
            chat.tools_port = Some(port);
            chat.tools_rx = Some(rx);
            chat.tools_tx = Some(tx);
        }
    }
    let cli = find_cli();
    if chat.tools_port.is_none() {
        chat.notice(tr("ui.agent_chat_no_control"), true);
    } else if cli.is_none() {
        chat.notice(tr("ui.agent_chat_no_cli"), true);
    }
    let bridge = cli.zip(chat.tools_port).map(|(cli, port)| acp::McpBridge { cli, port });
    let wake = ctx.clone();
    let start = backend::StartContext { cwd: working_dir(), bridge, tools: chat.tools_tx.clone(), wake: Box::new(move || wake.request_repaint()) };
    match acp_client::AcpBackend::start(&argv, start) {
        Ok(b) => {
            chat.backend = Some(Box::new(b));
            chat.argv = argv;
            chat.state = Some(State::Starting);
            chat.auth.clear();
            chat.auth_status = None;
            chat.agent.clear();
        }
        Err(e) => {
            chat.notice(trf("ui.agent_chat_failed_start", &[("error", &e)]), true);
            chat.state = Some(State::Exited);
        }
    }
}

/// Stop the agent (and everything it started); the conversation stays.
pub fn disconnect(app: &mut SolveApp) {
    end_turn(app);
    let chat = &mut app.agent_chat;
    chat.stop_backend(false);
    if chat.state.is_some() {
        chat.state = Some(State::Off);
    }
}

/// The app is quitting: end the agent and its helpers before the window goes.
pub fn shutdown(app: &mut SolveApp) {
    end_turn(app);
    app.agent_chat.stop_backend(true);
}

/// Send what the user typed: now if the agent is ready, else once it is (starting it if needed).
pub fn send(app: &mut SolveApp, ctx: &egui::Context, text: String) {
    let text: String = text.chars().take(MAX_PROMPT).collect();
    if text.trim().is_empty() {
        return;
    }
    let image = app.agent_chat.attach_view && app.agent_chat.images;
    app.agent_chat.push(Item::User { text: text.clone(), image });
    match app.agent_chat.state() {
        State::Ready => start_turn(app, text, image),
        State::Busy => app.agent_chat.notice(tr("ui.agent_chat_wait_turn"), false),
        s => {
            app.agent_chat.queued = Some((text, image));
            #[cfg(not(target_arch = "wasm32"))]
            if !s.running() {
                connect(app, ctx);
            }
            #[cfg(target_arch = "wasm32")]
            let _ = (s, ctx);
        }
    }
}

fn start_turn(app: &mut SolveApp, text: String, image: bool) {
    let mut parts = vec![PromptPart::Text(text.clone())];
    if image && let Some(png) = crate::control::view_png(app, VIEW_IMAGE.0, VIEW_IMAGE.1) {
        parts.push(PromptPart::Image { mime: "image/png".into(), base64: backend::base64(&png) });
    }
    let label: String = text.lines().next().unwrap_or("").chars().take(48).collect();
    app.agent_chat.turn = Some(Turn {
        start_doc: app.session.doc.clone(),
        start_undo: app.session.undo.len(),
        changes: 0,
        requests: 0,
        limit_told: false,
        label,
    });
    app.agent_chat.last = Some((text, image));
    app.agent_chat.state = Some(State::Busy);
    if let Some(b) = app.agent_chat.backend.as_mut() {
        b.prompt(&parts);
    }
}

/// Ask the agent to stop the message it is on; requests waiting for approval are declined.
pub fn cancel(app: &mut SolveApp) {
    decline_all(app, "the user stopped the message");
    if let Some(b) = app.agent_chat.backend.as_mut() {
        b.cancel();
    }
    for i in &mut app.agent_chat.items {
        if let Item::Permission { answer: a @ None, .. } = i {
            *a = Some(tr("ui.agent_chat_cancel").to_string());
        }
    }
}

fn decline_all(app: &mut SolveApp, why: &str) {
    let ids: Vec<u64> = app.agent_chat.held.iter().map(|h| h.id).collect();
    for id in ids {
        approve_with(app, id, false, why);
    }
}

/// Allow or deny a held request.
pub fn approve(app: &mut SolveApp, id: u64, allow: bool) {
    approve_with(app, id, allow, "the user declined it");
}

fn approve_with(app: &mut SolveApp, id: u64, allow: bool, why: &str) {
    let chat = &mut app.agent_chat;
    let Some(i) = chat.held.iter().position(|h| h.id == id) else { return };
    let h = chat.held.remove(i);
    for item in &mut chat.items {
        if let Item::Approval { id: x, answer, .. } = item
            && *x == id
        {
            *answer = Some(allow);
        }
    }
    if allow {
        app.control_held.push_back(h.req);
    } else {
        let _ = h.req.reply.send(json!({"ok": false, "error": format!("not run: {why}")}));
    }
}

/// The message is answered: its changes become one undo step, and what still waits is declined.
fn end_turn(app: &mut SolveApp) {
    decline_all(app, "the message ended");
    let Some(turn) = app.agent_chat.turn.take() else { return };
    let steps = merge_undo(&mut app.session.undo, &turn.start_doc, turn.start_undo, &trf("ui.agent_chat_undo_label", &[("message", &turn.label)]));
    if steps > 0 {
        let doc = app.session.doc.clone();
        app.agent_chat.push(Item::Changes { steps, doc, undone: false });
    }
}

/// Merge the undo steps made since `start_doc` (at depth `start_undo`) into one, labelled `label`;
/// how many steps it took in (0: none were made, or the history no longer reaches that far).
fn merge_undo(undo: &mut Vec<solidcraft_engine::Snapshot>, start_doc: &Arc<solidcraft_engine::doc::Document>, start_undo: usize, label: &str) -> usize {
    // The first change of the turn saved the design as it was when the turn began. When the
    // history is full its oldest steps go, so look for it at or below where it was put.
    let Some(first) = (0..undo.len().min(start_undo + 1)).rev().find(|&i| undo.get(i).is_some_and(|s| Arc::ptr_eq(&s.doc, start_doc))) else {
        return 0;
    };
    let steps = undo.len() - first;
    undo.truncate(first + 1);
    if let Some(s) = undo.get_mut(first) {
        s.label = label.to_string();
    }
    steps
}

/// Undo a message's changes from the conversation (only while they are the last thing done).
pub fn undo_changes(app: &mut SolveApp, index: usize) {
    let Some(Item::Changes { doc, undone, .. }) = app.agent_chat.items.get(index) else { return };
    if *undone || !Arc::ptr_eq(doc, &app.session.doc) {
        return;
    }
    if app.run("edit.undo", json!({})).is_ok()
        && let Some(Item::Changes { undone, .. }) = app.agent_chat.items.get_mut(index)
    {
        *undone = true;
    }
}

/// The requests the agent's tools sent through the chat's endpoint: each one passes the gate.
fn pump_tools(app: &mut SolveApp) {
    let Some(rx) = app.agent_chat.tools_rx.take() else { return };
    let limit = app.preferences.agent_chat_turn_limit.clamp(1, 1000);
    for mut req in rx.try_iter().take(64) {
        let refuse = |req: ControlRequest, e: String| {
            let _ = req.reply.send(json!({"ok": false, "error": e}));
        };
        let undo_len = app.session.undo.len();
        let Some(turn) = app.agent_chat.turn.as_mut() else {
            refuse(req, "the agent chat has no message in progress".into());
            continue;
        };
        turn.requests += 1;
        if turn.requests > gate::MAX_REQUESTS_PER_TURN {
            refuse(req, format!("too many requests in one message (at most {})", gate::MAX_REQUESTS_PER_TURN));
            continue;
        }
        // Requests without folders of their own stay inside the chat's working folder.
        if let Some(o) = req.params.as_object_mut()
            && !o.contains_key("allow_paths")
        {
            o.insert("allow_paths".into(), json!([working_dir().to_string_lossy()]));
        }
        let undo_left = undo_len.saturating_sub(turn.start_undo);
        match gate::request(&req.method, &req.params, undo_left) {
            gate::Verdict::Run { change } => {
                if change {
                    if turn.changes >= limit {
                        let first = !turn.limit_told;
                        turn.limit_told = true;
                        refuse(req, format!("the limit of {limit} changes for one message is reached: tell the user what is left to do"));
                        if first {
                            app.agent_chat.notice(trf("ui.agent_chat_limit", &[("limit", &limit.to_string())]), false);
                        }
                        continue;
                    }
                    turn.changes += 1;
                }
                app.control_held.push_back(req);
            }
            gate::Verdict::Ask(title) => {
                let chat = &mut app.agent_chat;
                chat.next_approval += 1;
                let id = chat.next_approval;
                chat.push(Item::Approval { id, title, answer: None });
                chat.held.push(Held { id, req, since: crate::now_ms() });
            }
            gate::Verdict::Refuse(e) => {
                app.agent_chat.notice(trf("ui.agent_chat_refused", &[("reason", &e)]), false);
                refuse(req, e);
            }
        }
    }
    app.agent_chat.tools_rx = Some(rx);
    // Questions nobody answered: the agent stopped waiting long ago.
    let now = crate::now_ms();
    let stale: Vec<u64> = app.agent_chat.held.iter().filter(|h| now - h.since > APPROVAL_TIMEOUT_MS).map(|h| h.id).collect();
    for id in stale {
        approve_with(app, id, false, "nobody answered in time");
    }
}

/// Read what the agent sent since the last frame, its tool requests, and the agent search.
pub fn poll(app: &mut SolveApp, ctx: &egui::Context) {
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(rx) = &app.agent_chat.detecting
        && let Ok(d) = rx.try_recv()
    {
        app.agent_chat.detection = Some(d);
        app.agent_chat.detecting = None;
    }
    // Turned off in Preferences: nothing keeps running.
    if !app.preferences.agent_chat_enabled && app.agent_chat.backend.is_some() {
        disconnect(app);
    }
    let events = app.agent_chat.backend.as_mut().map(|b| b.poll()).unwrap_or_default();
    for e in events {
        let ready = matches!(e, Event::Ready { .. });
        if app.agent_chat.on_event(e).is_some() {
            end_turn(app);
        }
        if ready && let Some((text, image)) = app.agent_chat.queued.take() {
            start_turn(app, text, image);
        }
    }
    pump_tools(app);
    if !app.agent_chat.held.is_empty() || app.agent_chat.state() == State::Busy {
        ctx.request_repaint_after(std::time::Duration::from_millis(250));
    }
}

/// Look for installed agents in the background (cached; Refresh looks again).
#[cfg(not(target_arch = "wasm32"))]
pub fn refresh(app: &mut SolveApp, ctx: &egui::Context) {
    if app.agent_chat.detecting.is_some() {
        return;
    }
    let (tx, rx) = std::sync::mpsc::channel();
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        let _ = tx.send(discover::detect());
        ctx.request_repaint();
    });
    app.agent_chat.detecting = Some(rx);
}

/// `ui.agent_chat` on the control channel (tests and screenshots): `enabled`, `command`, `open`,
/// `side`, `minimized`, `width`, `refresh`, `connect`, `stop`, `send`, `attach_view`,
/// `permission` (an option id, or `"cancel"`), `approve` (`true`/`false`, the oldest held
/// request), `cancel`, `undo` (the newest message's changes); returns the panel's state.
pub fn control(app: &mut SolveApp, ctx: &egui::Context, p: &Value) -> Result<Value, String> {
    if let Some(e) = p.get("enabled").and_then(Value::as_bool) {
        app.preferences.agent_chat_enabled = e;
    }
    if let Some(c) = p.get("command").and_then(Value::as_str) {
        app.preferences.agent_command = c.chars().take(4096).collect();
    }
    if let Some(o) = p.get("open").and_then(Value::as_bool) {
        app.agent_chat.open = o;
    }
    if let Some(s) = p.get("side").and_then(Value::as_str) {
        if !matches!(s, "left" | "right") {
            return Err("side must be left or right".into());
        }
        app.preferences.agent_chat_side = s.into();
    }
    if let Some(m) = p.get("minimized").and_then(Value::as_bool) {
        app.preferences.agent_chat_minimized = m;
    }
    if let Some(w) = p.get("width").and_then(Value::as_f64).filter(|w| w.is_finite()) {
        app.preferences.agent_chat_width = (w as f32).clamp(panel::MIN_WIDTH, panel::MAX_WIDTH);
        panel::forget_width(ctx);
    }
    if let Some(a) = p.get("attach_view").and_then(Value::as_bool) {
        app.agent_chat.attach_view = a;
    }
    if let Some(pick) = p.get("picker").and_then(Value::as_bool) {
        app.agent_chat.picking = Some(pick);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        if p.get("refresh").and_then(Value::as_bool) == Some(true) {
            refresh(app, ctx);
        }
        if p.get("stop").and_then(Value::as_bool) == Some(true) {
            disconnect(app);
        }
        if p.get("connect").and_then(Value::as_bool) == Some(true) {
            if !app.preferences.agent_chat_enabled {
                return Err("agent chat is turned off in Preferences".into());
            }
            connect(app, ctx);
        }
    }
    if let Some(text) = p.get("send").and_then(Value::as_str) {
        if !app.preferences.agent_chat_enabled {
            return Err("agent chat is turned off in Preferences".into());
        }
        send(app, ctx, text.to_string());
    }
    if let Some(o) = p.get("permission").and_then(Value::as_str) {
        let Some(n) = app.agent_chat.items.iter().rposition(|i| matches!(i, Item::Permission { answer: None, .. })) else {
            return Err("no permission request is waiting".into());
        };
        app.agent_chat.answer(n, (o != "cancel").then_some(o));
    }
    if let Some(allow) = p.get("approve").and_then(Value::as_bool) {
        let Some(id) = app.agent_chat.held.first().map(|h| h.id) else { return Err("no request is waiting for approval".into()) };
        approve(app, id, allow);
    }
    if p.get("cancel").and_then(Value::as_bool) == Some(true) {
        cancel(app);
    }
    if p.get("undo").and_then(Value::as_bool) == Some(true) {
        let Some(n) = app.agent_chat.items.iter().rposition(|i| matches!(i, Item::Changes { .. })) else { return Err("no changes to undo".into()) };
        undo_changes(app, n);
    }
    Ok(status(app))
}

pub fn status(app: &SolveApp) -> Value {
    let c = &app.agent_chat;
    let items: Vec<Value> = c
        .items
        .iter()
        .map(|i| match i {
            Item::User { text, image } => json!({"user": text, "image": image}),
            Item::Agent { thought, text } => json!({"agent": text, "thought": thought}),
            Item::Tool { id, title, kind, status, output } => {
                json!({"tool": id, "title": title, "kind": kind, "status": status.map(|s| format!("{s:?}")), "output": output})
            }
            Item::Plan(p) => json!({"plan": p}),
            Item::Permission { title, options, answer, .. } => {
                json!({"permission": title, "options": options.iter().map(|o| json!({"id": o.id, "name": o.name, "kind": o.kind})).collect::<Vec<_>>(), "answer": answer})
            }
            Item::Approval { title, answer, .. } => json!({"approval": title, "answer": answer}),
            Item::Changes { steps, undone, .. } => json!({"changes": steps, "undone": undone}),
            Item::Notice { text, error } => json!({"notice": text, "error": error}),
        })
        .collect();
    #[cfg(not(target_arch = "wasm32"))]
    let detection = c.detection.as_ref().map(|d| {
        json!({
            "found": d.found.iter().map(|f| json!({"id": f.id, "name": f.name, "argv": f.argv, "version": f.version, "login": f.login.id()})).collect::<Vec<_>>(),
            "missing": d.missing.iter().map(|m| json!({"id": m.id, "install": m.install})).collect::<Vec<_>>(),
            "ms": d.took_ms,
        })
    });
    #[cfg(target_arch = "wasm32")]
    let detection: Option<Value> = None;
    let pid = c.backend.as_ref().and_then(|b| b.pid());
    json!({
        "enabled": app.preferences.agent_chat_enabled,
        "open": c.open,
        "side": app.preferences.agent_chat_side,
        "minimized": app.preferences.agent_chat_minimized,
        "width": app.preferences.agent_chat_width,
        "command": app.preferences.agent_command,
        "state": c.state().id(),
        "agent": c.agent,
        "images": c.images,
        "authStatus": c.auth_status,
        "authMethods": c.auth.iter().map(|m| json!({"id": m.id, "name": m.name, "kind": match &m.kind { acp::AuthKind::Agent => "agent", acp::AuthKind::Terminal { .. } => "terminal", acp::AuthKind::EnvVar { .. } => "env_var" }})).collect::<Vec<_>>(),
        "toolsPort": c.tools_port,
        "pid": pid,
        "waiting": c.waiting(),
        "turn": c.turn.as_ref().map(|t| json!({"changes": t.changes, "requests": t.requests})),
        "detection": detection,
        "items": items,
    })
}

#[cfg(test)]
mod tests;
