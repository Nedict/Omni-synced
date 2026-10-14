//! Agent chat: a side panel that talks to a coding agent the user already has (Claude Code,
//! Codex, Gemini CLI, …) over the Agent Client Protocol, and gives that agent SolidCraft's MCP
//! server, bridged to this window, as its way to read and change the open design
//! (docs/design/in-app-agent-chat.md).
//!
//! Desktop only: the agent is a child process. The panel is hidden unless the preference is on.
//! SolidCraft never handles the agent's credentials: an agent that needs a login says so, and the
//! panel shows the agent's own login command for the user to run.

// The web build shows only a note: the process side is desktop only.
#![cfg_attr(target_arch = "wasm32", allow(dead_code, unused_imports))]

pub mod acp;
#[cfg(not(target_arch = "wasm32"))]
pub mod discover;
#[cfg(not(target_arch = "wasm32"))]
mod process;

use egui::RichText;
use serde_json::{Value, json};

use crate::SolveApp;
use crate::i18n::{format as trf, tr};
use acp::{AuthKind, Incoming, PermissionOption, ToolStatus};

/// Most entries kept in the conversation; older ones are dropped.
const MAX_ITEMS: usize = 2000;
/// Longest prompt sent.
const MAX_PROMPT: usize = 100_000;

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
}

#[derive(Clone, Debug)]
pub enum Item {
    User(String),
    Agent { thought: bool, text: String },
    Tool { id: String, title: String, kind: String, status: Option<ToolStatus>, output: Option<String> },
    Plan(Vec<(String, String)>),
    Permission { rpc: Value, title: String, options: Vec<PermissionOption>, answer: Option<String> },
    Notice { text: String, error: bool },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pending {
    Initialize,
    SessionNew,
    Prompt,
    Authenticate,
}

/// Starts the app's control server for the agent's MCP bridge when it isn't running yet
/// (the desktop host provides it): the port it listens on and its request channel.
pub type ControlStarter = Box<dyn FnMut() -> Option<(u16, std::sync::mpsc::Receiver<crate::ControlRequest>)>>;

#[derive(Default)]
pub struct AgentChat {
    /// The panel is shown (toggled from the application bar while the preference is on).
    pub open: bool,
    pub items: Vec<Item>,
    pub input: String,
    state: Option<State>,
    agent: String,
    auth: Vec<acp::AuthMethod>,
    auth_status: Option<String>,
    session: Option<String>,
    next_id: u64,
    pending: std::collections::HashMap<u64, Pending>,
    /// A prompt typed before the session was ready.
    queued: Option<String>,
    /// The command the running agent was started with.
    argv: Vec<String>,
    /// The control port the agent's MCP server bridges to.
    pub control_port: Option<u16>,
    /// Desktop host hook: start the control server on demand.
    pub control_starter: Option<ControlStarter>,
    custom: bool,
    custom_text: String,
    #[cfg(not(target_arch = "wasm32"))]
    proc: Option<process::AgentProcess>,
    #[cfg(not(target_arch = "wasm32"))]
    detection: Option<discover::Detection>,
    #[cfg(not(target_arch = "wasm32"))]
    detecting: Option<std::sync::mpsc::Receiver<discover::Detection>>,
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
    fn next(&mut self, what: Pending) -> u64 {
        self.next_id += 1;
        self.pending.insert(self.next_id, what);
        self.next_id
    }
    fn send(&mut self, msg: &Value) {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(p) = &self.proc {
            p.send(msg);
        }
        #[cfg(target_arch = "wasm32")]
        let _ = msg;
    }
    fn request(&mut self, what: Pending, method: &str, params: Value) {
        let id = self.next(what);
        self.send(&acp::request(id, method, params));
    }

    /// Read one message from the agent.
    fn handle(&mut self, msg: Incoming, cwd: &std::path::Path, bridge: Option<&acp::McpBridge>) {
        match msg {
            Incoming::Reply { id, result } => {
                let Some(what) = self.pending.remove(&id) else { return };
                match (what, result) {
                    (Pending::Initialize, Ok(v)) => {
                        self.agent = acp::agent_info(&v);
                        self.auth = acp::auth_methods(&v);
                        self.request(Pending::SessionNew, "session/new", acp::session_new_params(cwd, bridge));
                    }
                    (Pending::SessionNew, Ok(v)) => match v.get("sessionId").and_then(Value::as_str) {
                        Some(s) => {
                            self.session = Some(s.to_string());
                            self.state = Some(State::Ready);
                            if let Some(q) = self.queued.take() {
                                // Already shown when it was typed.
                                self.state = Some(State::Busy);
                                self.request(Pending::Prompt, "session/prompt", acp::prompt_params(s, &q));
                            }
                        }
                        None => self.fail("session/new: no sessionId"),
                    },
                    (Pending::Prompt, Ok(v)) => {
                        self.state = Some(State::Ready);
                        let reason = v.get("stopReason").and_then(Value::as_str).unwrap_or("");
                        if !matches!(reason, "end_turn" | "") {
                            self.notice(format!("({reason})"), false);
                        }
                        // A question asked during the turn goes next (already shown).
                        if let (Some(q), Some(s)) = (self.queued.take(), self.session.clone()) {
                            self.state = Some(State::Busy);
                            self.request(Pending::Prompt, "session/prompt", acp::prompt_params(&s, &q));
                        }
                    }
                    (Pending::Authenticate, Ok(_)) => {
                        // Signed in: try the session again.
                        self.state = Some(State::Starting);
                        self.request(Pending::SessionNew, "session/new", acp::session_new_params(cwd, bridge));
                    }
                    (what, Err((code, message))) => {
                        if code == acp::AUTH_REQUIRED || message.to_ascii_lowercase().contains("auth") {
                            if what == Pending::Prompt {
                                // Keep what was asked, for when the login is done.
                                self.queued = self.items.iter().rev().find_map(|i| if let Item::User(t) = i { Some(t.clone()) } else { None });
                            }
                            self.state = Some(State::NeedsAuth);
                            self.session = None;
                        } else {
                            self.state = Some(if self.session.is_some() { State::Ready } else { State::Exited });
                            self.notice(message, true);
                        }
                    }
                }
            }
            Incoming::Text { thought, text } => {
                if let Some(Item::Agent { thought: t, text: last }) = self.items.last_mut()
                    && *t == thought
                {
                    last.push_str(&text);
                } else {
                    self.push(Item::Agent { thought, text });
                }
            }
            Incoming::Tool { id, title, kind, status, output } => {
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
            Incoming::Plan(entries) => {
                // A plan is replaced whole: update this turn's plan where it is.
                let turn = self.items.iter().rposition(|i| matches!(i, Item::User(_))).unwrap_or(0);
                if let Some(Item::Plan(p)) = self.items.iter_mut().skip(turn).rev().find(|i| matches!(i, Item::Plan(_))) {
                    *p = entries;
                } else {
                    self.push(Item::Plan(entries));
                }
            }
            Incoming::Permission { rpc, title, options } => self.push(Item::Permission { rpc, title, options, answer: None }),
            Incoming::AuthStatus(label) => self.auth_status = Some(label),
            Incoming::Unsupported { rpc, method } => {
                self.send(&acp::reply_error(rpc, acp::METHOD_NOT_FOUND, &format!("SolidCraft does not serve `{method}`")));
            }
            Incoming::Ignored => {}
        }
    }

    fn fail(&mut self, e: impl Into<String>) {
        self.notice(e, true);
        self.stop();
        self.state = Some(State::Exited);
    }

    /// Send what the user typed (queued until the session is ready).
    pub fn prompt(&mut self, text: String) {
        let text: String = text.chars().take(MAX_PROMPT).collect();
        if text.trim().is_empty() {
            return;
        }
        match (&self.session, self.state()) {
            (Some(s), State::Ready) => {
                let params = acp::prompt_params(s, &text);
                self.push(Item::User(text));
                self.state = Some(State::Busy);
                self.request(Pending::Prompt, "session/prompt", params);
            }
            _ => {
                self.push(Item::User(text.clone()));
                self.queued = Some(text);
            }
        }
    }

    /// Ask the agent to stop the turn it is on.
    pub fn cancel(&mut self) {
        if let Some(s) = self.session.clone() {
            // Open permission prompts are answered "cancelled", as the protocol asks.
            let open: Vec<usize> =
                self.items.iter().enumerate().filter(|(_, i)| matches!(i, Item::Permission { answer: None, .. })).map(|(n, _)| n).collect();
            for n in open {
                self.answer(n, None);
            }
            self.send(&acp::notification("session/cancel", json!({"sessionId": s})));
        }
    }

    /// Answer the permission prompt at `index` with an option id (`None`: cancelled).
    pub fn answer(&mut self, index: usize, option: Option<&str>) {
        let Some(Item::Permission { rpc, options, answer, .. }) = self.items.get_mut(index) else { return };
        if answer.is_some() {
            return;
        }
        let label =
            option.and_then(|o| options.iter().find(|x| x.id == o)).map_or_else(|| tr("ui.agent_chat_cancel").to_string(), |o| o.name.clone());
        *answer = Some(label);
        let msg = acp::permission_reply(rpc.clone(), option);
        self.send(&msg);
    }

    /// Ask the agent to sign in by itself (an "agent" login method: it opens the browser).
    pub fn authenticate(&mut self, method: &str) {
        self.request(Pending::Authenticate, "authenticate", json!({"methodId": method}));
        self.state = Some(State::Starting);
    }

    pub fn stop(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.proc = None;
        }
        self.session = None;
        self.pending.clear();
        if self.state.is_some() {
            self.state = Some(State::Off);
        }
    }
}

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

/// Where the agent works: the open design's folder, else SolidCraft's own agent folder.
#[cfg(not(target_arch = "wasm32"))]
fn working_dir(app: &SolveApp) -> std::path::PathBuf {
    if let Some(dir) =
        app.session.path.as_deref().map(std::path::Path::new).and_then(std::path::Path::parent).filter(|d| d.is_absolute() && d.is_dir())
    {
        return dir.to_path_buf();
    }
    let d = solidcraft_engine::recovery::data_dir().map(|d| d.join("agent")).unwrap_or_else(|| std::env::temp_dir().join("solidcraft-agent"));
    let _ = std::fs::create_dir_all(&d);
    d
}

#[cfg(not(target_arch = "wasm32"))]
fn bridge(app: &SolveApp) -> Option<acp::McpBridge> {
    Some(acp::McpBridge { cli: find_cli()?, port: app.agent_chat.control_port? })
}

/// Start the agent: the control server for its MCP bridge if needed, the process, `initialize`.
#[cfg(not(target_arch = "wasm32"))]
pub fn connect(app: &mut SolveApp, ctx: &egui::Context) {
    app.agent_chat.stop();
    let argv = chosen_command(app);
    if argv.is_empty() {
        app.agent_chat.notice(tr("ui.agent_chat_none_found"), true);
        return;
    }
    if app.agent_chat.control_port.is_none()
        && let Some(start) = app.agent_chat.control_starter.as_mut()
        && let Some((port, rx)) = start()
    {
        app.agent_chat.control_port = Some(port);
        app.add_control(rx);
    }
    if app.agent_chat.control_port.is_none() {
        app.agent_chat.notice(tr("ui.agent_chat_no_control"), true);
    } else if find_cli().is_none() {
        app.agent_chat.notice(tr("ui.agent_chat_no_cli"), true);
    }
    let cwd = working_dir(app);
    let wake = ctx.clone();
    match process::AgentProcess::spawn(&argv, &cwd, move || wake.request_repaint()) {
        Ok(p) => {
            let chat = &mut app.agent_chat;
            chat.proc = Some(p);
            chat.argv = argv;
            chat.state = Some(State::Starting);
            chat.auth.clear();
            chat.auth_status = None;
            chat.request(Pending::Initialize, "initialize", acp::initialize_params(env!("CARGO_PKG_VERSION")));
        }
        Err(e) => {
            app.agent_chat.notice(trf("ui.agent_chat_failed_start", &[("error", &e)]), true);
            app.agent_chat.state = Some(State::Exited);
        }
    }
}

/// Read what the agent sent since the last frame, and the agent detection's result.
pub fn poll(app: &mut SolveApp) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Some(rx) = &app.agent_chat.detecting
            && let Ok(d) = rx.try_recv()
        {
            app.agent_chat.detection = Some(d);
            app.agent_chat.detecting = None;
        }
        let Some(p) = &app.agent_chat.proc else { return };
        let mut msgs = Vec::new();
        let mut exited = None;
        for _ in 0..512 {
            match p.from_agent.try_recv() {
                Ok(process::FromAgent::Line(l)) => msgs.push(l),
                Ok(process::FromAgent::Exited(tail)) => {
                    exited = Some(tail);
                    break;
                }
                Err(_) => break,
            }
        }
        if msgs.is_empty() && exited.is_none() {
            return;
        }
        let cwd = working_dir(app);
        let bridge = bridge(app);
        for l in msgs {
            if let Some(m) = acp::parse(&l) {
                app.agent_chat.handle(m, &cwd, bridge.as_ref());
            }
        }
        if let Some(tail) = exited {
            let chat = &mut app.agent_chat;
            chat.proc = None;
            chat.session = None;
            chat.pending.clear();
            chat.state = Some(State::Exited);
            let last: Vec<String> = tail.into_iter().rev().take(6).collect::<Vec<_>>().into_iter().rev().collect();
            let detail = if last.is_empty() { String::new() } else { format!("\n{}", last.join("\n")) };
            chat.notice(format!("{}{detail}", tr("ui.agent_chat_exited")), true);
        }
    }
    #[cfg(target_arch = "wasm32")]
    let _ = app;
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

/// A custom command as the picker shows it: the program's file name and its arguments, cut short.
fn short_command(line: &str) -> String {
    let argv = acp::split_command(line);
    let mut parts: Vec<String> = argv.iter().map(|a| a.rsplit(['/', '\\']).next().unwrap_or(a).to_string()).collect();
    if parts.is_empty() {
        parts.push(line.to_string());
    }
    let s = parts.join(" ");
    if s.chars().count() > 40 { format!("{}…", s.chars().take(40).collect::<String>()) } else { s }
}

/// Quote an argument for the command field when it holds spaces.
fn quote(a: &str) -> String {
    if a.is_empty() || a.contains(char::is_whitespace) || a.contains('"') { format!("\"{}\"", a.replace('"', "\\\"")) } else { a.to_string() }
}

/// The application-bar button and the panel. Call before the central viewport is laid out.
pub fn show(app: &mut SolveApp, ui: &mut egui::Ui) {
    poll(app);
    if !app.preferences.agent_chat_enabled || !app.agent_chat.open {
        return;
    }
    let t = crate::theme::Tokens::get();
    egui::Panel::right("sc_agent_chat")
        .default_size(360.0)
        .size_range(260.0..=720.0)
        .resizable(true)
        .frame(egui::Frame::NONE.fill(t.panel).inner_margin(egui::Margin::symmetric(8, 6)))
        .show(ui, |ui| panel(app, ui));
}

#[cfg(target_arch = "wasm32")]
fn panel(_app: &mut SolveApp, ui: &mut egui::Ui) {
    ui.label(tr("ui.agent_chat_desktop_only"));
}

#[cfg(not(target_arch = "wasm32"))]
fn panel(app: &mut SolveApp, ui: &mut egui::Ui) {
    let t = crate::theme::Tokens::get();
    if app.agent_chat.detection.is_none() && app.agent_chat.detecting.is_none() {
        refresh(app, ui.ctx());
    }
    let state = app.agent_chat.state();
    // Header: title, state, connect/stop, close.
    ui.horizontal(|ui| {
        ui.label(RichText::new(tr("ui.agent_chat")).strong().size(14.0));
        let color = match state {
            State::Ready => t.accent,
            State::NeedsAuth | State::Exited => t.warning,
            _ => t.text_dim,
        };
        ui.label(RichText::new(state.label()).color(color).small());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("×").clicked() {
                app.agent_chat.open = false;
            }
            let running = matches!(state, State::Starting | State::Ready | State::Busy | State::NeedsAuth);
            if running {
                if ui.button(tr("ui.agent_chat_stop")).clicked() {
                    app.agent_chat.stop();
                }
            } else if ui.button(tr("ui.agent_chat_connect")).clicked() {
                connect(app, ui.ctx());
            }
        });
    });
    picker(app, ui);
    if !app.agent_chat.agent.is_empty() || app.agent_chat.auth_status.is_some() {
        let mut s = app.agent_chat.agent.clone();
        if let Some(a) = &app.agent_chat.auth_status {
            s = format!("{s} · {a}");
        }
        ui.label(RichText::new(s).color(t.text_dim));
    }
    if let Some(port) = app.agent_chat.control_port
        && app.agent_chat.proc.is_some()
    {
        ui.label(RichText::new(trf("ui.agent_chat_tools", &[("port", &port.to_string())])).color(t.text_dim));
    }
    if state == State::NeedsAuth {
        auth_box(app, ui);
    }
    ui.separator();
    // The input sits at the bottom; the conversation takes the rest.
    let input_h = 86.0;
    let avail = (ui.available_height() - input_h).max(80.0);
    egui::ScrollArea::vertical().max_height(avail).auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
        conversation(app, ui);
    });
    ui.separator();
    let busy = state == State::Busy;
    let edit = egui::TextEdit::multiline(&mut app.agent_chat.input)
        .hint_text(tr("ui.agent_chat_placeholder"))
        .desired_rows(2)
        .desired_width(f32::INFINITY)
        .id_salt("sc_agent_chat_input");
    let r = ui.add(edit);
    crate::scenario::publish_rect("agent_chat:input", r.rect);
    let enter = r.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift);
    ui.horizontal(|ui| {
        let send = ui.add_enabled(!busy, egui::Button::new(tr("ui.agent_chat_send"))).clicked();
        if (send || (enter && !busy)) && !app.agent_chat.input.trim().is_empty() {
            let text = std::mem::take(&mut app.agent_chat.input).trim_end_matches('\n').to_string();
            if app.agent_chat.proc.is_none() {
                connect(app, ui.ctx());
            }
            app.agent_chat.prompt(text);
        }
        if busy && ui.button(tr("ui.agent_chat_cancel")).clicked() {
            app.agent_chat.cancel();
        }
    });
}

/// The agent picker: the agents found (version, whether they look signed in), a custom
/// command, Refresh, and install hints for the ones missing.
#[cfg(not(target_arch = "wasm32"))]
fn picker(app: &mut SolveApp, ui: &mut egui::Ui) {
    let t = crate::theme::Tokens::get();
    let current = app.preferences.agent_command.clone();
    let found: Vec<discover::Found> = app.agent_chat.detection.as_ref().map(|d| d.found.clone()).unwrap_or_default();
    let line = |f: &discover::Found| f.argv.iter().map(|a| quote(a)).collect::<Vec<_>>().join(" ");
    let label = |f: &discover::Found| {
        let mut s = f.name.to_string();
        if let Some(v) = &f.version {
            s = format!("{s}  {v}");
        }
        match f.login {
            discover::Login::LooksLoggedIn => format!("{s}  · {}", tr("ui.agent_chat_signed_in")),
            discover::Login::NotLoggedIn => format!("{s}  · {}", tr("ui.agent_chat_not_signed_in")),
            discover::Login::Unknown => s,
        }
    };
    let selected = if current.trim().is_empty() {
        found.first().map(label).unwrap_or_else(|| tr("ui.agent_chat_none_found").to_string())
    } else {
        found.iter().find(|f| line(f) == current).map(label).unwrap_or_else(|| short_command(&current))
    };
    ui.horizontal(|ui| {
        let w = (ui.available_width() - 70.0).max(120.0);
        egui::ComboBox::from_id_salt("sc_agent_pick").width(w).selected_text(selected).show_ui(ui, |ui| {
            for f in &found {
                let l = line(f);
                if ui.selectable_label(current == l, label(f)).clicked() {
                    app.preferences.agent_command = l;
                    app.agent_chat.custom = false;
                }
            }
            if ui.selectable_label(app.agent_chat.custom, tr("ui.agent_chat_add_custom")).clicked() {
                app.agent_chat.custom = true;
                app.agent_chat.custom_text = current.clone();
            }
        });
        let refreshing = app.agent_chat.detecting.is_some();
        if ui.add_enabled(!refreshing, egui::Button::new(tr("ui.agent_chat_refresh"))).clicked() {
            refresh(app, ui.ctx());
        }
    });
    if app.agent_chat.detecting.is_some() {
        ui.label(RichText::new(tr("ui.agent_chat_detecting")).color(t.text_dim));
    }
    if app.agent_chat.custom {
        ui.horizontal(|ui| {
            ui.label(tr("ui.agent_chat_custom"));
            let r = ui.add(egui::TextEdit::singleline(&mut app.agent_chat.custom_text).desired_width(f32::INFINITY).hint_text("my-agent --acp"));
            if r.lost_focus() {
                app.preferences.agent_command = app.agent_chat.custom_text.trim().to_string();
            }
        });
    }
    let missing: Vec<(&str, &str, &str)> = app.agent_chat.detection.as_ref().map(|d| d.missing.clone()).unwrap_or_default();
    if !missing.is_empty() {
        let heading = if found.is_empty() { tr("ui.agent_chat_none_found") } else { tr("ui.agent_chat_not_installed") };
        egui::CollapsingHeader::new(RichText::new(heading)).id_salt("sc_agent_missing").default_open(found.is_empty()).show(ui, |ui| {
            for (_, name, hint) in missing {
                ui.label(RichText::new(name).strong());
                ui.add(egui::Label::new(RichText::new(hint).monospace().size(11.0).color(t.text_dim)).selectable(true));
            }
        });
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn auth_box(app: &mut SolveApp, ui: &mut egui::Ui) {
    let t = crate::theme::Tokens::get();
    egui::Frame::group(ui.style()).stroke(egui::Stroke::new(1.0, t.warning)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new(tr("ui.agent_chat_auth_needed")));
        let methods = app.agent_chat.auth.clone();
        let base = app.agent_chat.argv.iter().map(|a| quote(a)).collect::<Vec<_>>().join(" ");
        for m in methods {
            ui.add_space(4.0);
            ui.label(
                RichText::new(if m.description.trim().is_empty() { m.name.clone() } else { format!("{} — {}", m.name, m.description.trim()) })
                    .strong(),
            );
            match &m.kind {
                AuthKind::Terminal { args, .. } => {
                    ui.label(RichText::new(tr("ui.agent_chat_auth_terminal")));
                    let cmd = format!("{base} {}", args.iter().map(|a| quote(a)).collect::<Vec<_>>().join(" "));
                    ui.horizontal(|ui| {
                        ui.add(egui::Label::new(RichText::new(&cmd).monospace().size(11.0)).selectable(true).wrap());
                        if ui.small_button(tr("ui.agent_chat_copy")).clicked() {
                            ui.ctx().copy_text(cmd.clone());
                        }
                    });
                }
                AuthKind::Agent => {
                    if ui.button(trf("ui.agent_chat_auth_agent", &[("name", &m.name)])).clicked() {
                        app.agent_chat.authenticate(&m.id);
                    }
                }
                AuthKind::EnvVar { vars } => {
                    ui.label(RichText::new(trf("ui.agent_chat_auth_env", &[("vars", &vars.join(", "))])));
                }
            }
        }
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn conversation(app: &mut SolveApp, ui: &mut egui::Ui) {
    let t = crate::theme::Tokens::get();
    let mut answer: Option<(usize, Option<String>)> = None;
    for (n, item) in app.agent_chat.items.iter().enumerate() {
        match item {
            Item::User(text) => {
                ui.add_space(6.0);
                egui::Frame::NONE.fill(t.accent_soft).corner_radius(6.0).inner_margin(egui::Margin::same(6)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.add(egui::Label::new(RichText::new(text).color(t.text)).selectable(true).wrap());
                });
            }
            Item::Agent { thought, text } => {
                let rt = if *thought { RichText::new(text).italics().color(t.text_dim) } else { RichText::new(text).color(t.text) };
                ui.add(egui::Label::new(rt).selectable(true).wrap());
            }
            Item::Tool { title, kind, status, output, .. } => {
                let (color, filled) = match status {
                    Some(ToolStatus::Completed) => (t.accent, true),
                    Some(ToolStatus::Failed) => (t.error, true),
                    Some(ToolStatus::InProgress) => (t.warning, true),
                    _ => (t.text_dim, false),
                };
                let head = if kind.is_empty() { title.clone() } else { format!("{title}  [{kind}]") };
                ui.horizontal(|ui| {
                    status_dot(ui, color, filled);
                    egui::CollapsingHeader::new(RichText::new(head).color(t.text)).id_salt(("sc_agent_tool", n)).show(ui, |ui| {
                        if let Some(o) = output {
                            ui.add(egui::Label::new(RichText::new(o).monospace().size(11.0)).selectable(true).wrap());
                        }
                    });
                });
            }
            Item::Plan(entries) => {
                for (c, s) in entries {
                    let (color, filled) = match s.as_str() {
                        "completed" => (t.accent, true),
                        "in_progress" => (t.warning, true),
                        _ => (t.text_dim, false),
                    };
                    ui.horizontal(|ui| {
                        status_dot(ui, color, filled);
                        ui.label(RichText::new(c).color(t.text_dim));
                    });
                }
            }
            Item::Permission { title, options, answer: given, .. } => {
                egui::Frame::group(ui.style()).stroke(egui::Stroke::new(1.0, t.accent)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(trf("ui.agent_chat_permission", &[("title", title)])).strong());
                    match given {
                        Some(a) => {
                            ui.label(RichText::new(trf("ui.agent_chat_answered", &[("option", a)])).color(t.text_dim));
                        }
                        None => {
                            ui.horizontal_wrapped(|ui| {
                                for o in options {
                                    let b = egui::Button::new(&o.name);
                                    let b = if o.kind.starts_with("allow") { b.fill(t.accent_soft) } else { b };
                                    let r = ui.add(b);
                                    crate::scenario::publish_handle(&format!("agent_chat:permission:{}", o.id), r.rect.center());
                                    if r.clicked() {
                                        answer = Some((n, Some(o.id.clone())));
                                    }
                                }
                            });
                        }
                    }
                });
            }
            Item::Notice { text, error } => {
                ui.label(RichText::new(text).color(if *error { t.error } else { t.text_dim }));
            }
        }
        ui.add_space(2.0);
    }
    if app.agent_chat.items.is_empty() {
        ui.add_space(12.0);
        ui.label(RichText::new(tr("ui.agent_chat_intro")).color(t.text_dim));
    }
    if let Some((n, o)) = answer {
        app.agent_chat.answer(n, o.as_deref());
    }
}

/// A small status mark drawn as a shape (no glyph a system font might lack): filled when the
/// step is running or done, hollow while it waits.
#[cfg(not(target_arch = "wasm32"))]
fn status_dot(ui: &mut egui::Ui, color: egui::Color32, filled: bool) {
    let (r, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
    if filled {
        ui.painter().circle_filled(r.center(), 4.0, color);
    } else {
        ui.painter().circle_stroke(r.center(), 3.5, egui::Stroke::new(1.2, color));
    }
}

/// `ui.agent_chat` on the control channel (tests and screenshots): `enabled`, `command`, `open`,
/// `refresh`, `connect`, `stop`, `send`, `permission` (an option id, or `"cancel"`), `cancel`;
/// returns the panel's state.
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
    #[cfg(not(target_arch = "wasm32"))]
    {
        if p.get("refresh").and_then(Value::as_bool) == Some(true) {
            refresh(app, ctx);
        }
        if p.get("stop").and_then(Value::as_bool) == Some(true) {
            app.agent_chat.stop();
        }
        if p.get("connect").and_then(Value::as_bool) == Some(true) {
            if !app.preferences.agent_chat_enabled {
                return Err("agent chat is turned off in Preferences".into());
            }
            connect(app, ctx);
        }
        if let Some(text) = p.get("send").and_then(Value::as_str) {
            if app.agent_chat.proc.is_none() {
                connect(app, ctx);
            }
            app.agent_chat.prompt(text.to_string());
        }
        if let Some(o) = p.get("permission").and_then(Value::as_str) {
            let Some(n) = app.agent_chat.items.iter().rposition(|i| matches!(i, Item::Permission { answer: None, .. })) else {
                return Err("no permission request is waiting".into());
            };
            app.agent_chat.answer(n, (o != "cancel").then_some(o));
        }
        if p.get("cancel").and_then(Value::as_bool) == Some(true) {
            app.agent_chat.cancel();
        }
    }
    #[cfg(target_arch = "wasm32")]
    let _ = ctx;
    Ok(status(app))
}

pub fn status(app: &SolveApp) -> Value {
    let c = &app.agent_chat;
    let items: Vec<Value> = c
        .items
        .iter()
        .map(|i| match i {
            Item::User(t) => json!({"user": t}),
            Item::Agent { thought, text } => json!({"agent": text, "thought": thought}),
            Item::Tool { id, title, kind, status, output } => {
                json!({"tool": id, "title": title, "kind": kind, "status": status.map(|s| format!("{s:?}")), "output": output})
            }
            Item::Plan(p) => json!({"plan": p}),
            Item::Permission { title, options, answer, .. } => {
                json!({"permission": title, "options": options.iter().map(|o| json!({"id": o.id, "name": o.name, "kind": o.kind})).collect::<Vec<_>>(), "answer": answer})
            }
            Item::Notice { text, error } => json!({"notice": text, "error": error}),
        })
        .collect();
    #[cfg(not(target_arch = "wasm32"))]
    let detection = c.detection.as_ref().map(|d| {
        json!({
            "found": d.found.iter().map(|f| json!({"id": f.id, "name": f.name, "argv": f.argv, "version": f.version, "login": format!("{:?}", f.login)})).collect::<Vec<_>>(),
            "missing": d.missing.iter().map(|(id, _, hint)| json!({"id": id, "install": hint})).collect::<Vec<_>>(),
            "ms": d.took_ms,
        })
    });
    #[cfg(target_arch = "wasm32")]
    let detection: Option<Value> = None;
    #[cfg(not(target_arch = "wasm32"))]
    let pid = c.proc.as_ref().map(process::AgentProcess::pid);
    #[cfg(target_arch = "wasm32")]
    let pid: Option<u32> = None;
    json!({
        "enabled": app.preferences.agent_chat_enabled,
        "open": c.open,
        "command": app.preferences.agent_command,
        "state": c.state().id(),
        "agent": c.agent,
        "authStatus": c.auth_status,
        "authMethods": c.auth.iter().map(|m| json!({"id": m.id, "name": m.name, "kind": match &m.kind { AuthKind::Agent => "agent", AuthKind::Terminal { .. } => "terminal", AuthKind::EnvVar { .. } => "env_var" }})).collect::<Vec<_>>(),
        "session": c.session,
        "controlPort": c.control_port,
        "pid": pid,
        "detection": detection,
        "items": items,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chat() -> AgentChat {
        AgentChat { state: Some(State::Ready), session: Some("s1".into()), ..Default::default() }
    }

    #[test]
    fn streamed_text_joins_and_tool_updates_change_their_call() {
        let mut c = chat();
        let cwd = std::path::Path::new("/tmp");
        for m in [
            Incoming::Text { thought: false, text: "Mak".into() },
            Incoming::Text { thought: false, text: "ing a box".into() },
            Incoming::Tool {
                id: "t".into(),
                title: Some("batch".into()),
                kind: Some("other".into()),
                status: Some(ToolStatus::Pending),
                output: None,
            },
            Incoming::Tool { id: "t".into(), title: None, kind: None, status: Some(ToolStatus::Completed), output: Some("ok".into()) },
        ] {
            c.handle(m, cwd, None);
        }
        assert_eq!(c.items.len(), 2);
        assert!(matches!(&c.items[0], Item::Agent { text, .. } if text == "Making a box"));
        assert!(
            matches!(&c.items[1], Item::Tool { title, status: Some(ToolStatus::Completed), output: Some(o), .. } if title == "batch" && o == "ok")
        );
    }

    #[test]
    fn an_auth_error_asks_for_login_and_keeps_the_question() {
        let mut c = chat();
        c.prompt("make a cube".into());
        assert_eq!(c.state(), State::Busy);
        let id = c.next_id;
        c.handle(Incoming::Reply { id, result: Err((acp::AUTH_REQUIRED, "Authentication required".into())) }, std::path::Path::new("/tmp"), None);
        assert_eq!(c.state(), State::NeedsAuth);
        assert_eq!(c.queued.as_deref(), Some("make a cube"));
    }

    #[test]
    fn custom_commands_show_short_in_the_picker() {
        assert_eq!(short_command("python3 -I /very/long/path/to/mock_agent.py"), "python3 -I mock_agent.py");
        assert!(short_command(&"x".repeat(100)).ends_with('…'));
    }

    #[test]
    fn a_plan_update_replaces_the_turns_plan() {
        let mut c = chat();
        let cwd = std::path::Path::new("/tmp");
        c.prompt("go".into());
        c.handle(Incoming::Plan(vec![("a".into(), "pending".into())]), cwd, None);
        c.handle(Incoming::Text { thought: false, text: "working".into() }, cwd, None);
        c.handle(Incoming::Plan(vec![("a".into(), "completed".into())]), cwd, None);
        let plans: Vec<_> = c.items.iter().filter(|i| matches!(i, Item::Plan(_))).collect();
        assert_eq!(plans.len(), 1);
        assert!(matches!(plans[0], Item::Plan(p) if p[0].1 == "completed"));
    }

    #[test]
    fn a_permission_is_answered_once() {
        let mut c = chat();
        let opts = vec![PermissionOption { id: "allow".into(), name: "Allow".into(), kind: "allow_once".into() }];
        c.handle(Incoming::Permission { rpc: json!(5), title: "batch".into(), options: opts }, std::path::Path::new("/tmp"), None);
        c.answer(0, Some("allow"));
        c.answer(0, None);
        assert!(matches!(&c.items[0], Item::Permission { answer: Some(a), .. } if a == "Allow"));
    }
}
