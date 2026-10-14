//! Running a turn on a worker thread, so the window keeps drawing while the model answers and
//! tools run. The worker reports through a channel the UI drains once per frame.

use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use crate::model::{Message, ToolDef, Usage};
use crate::net::{Cancel, HttpStream};
use crate::provider::Codec;
use crate::settings::Secret;
use crate::tool_loop::{Event, Finish, ToolHost, TurnConfig, TurnOutcome, run_turn};

/// Everything one turn needs, owned, so it can move to the worker.
pub struct Job {
    pub codec: Arc<dyn Codec>,
    pub http: HttpStream,
    pub base_url: String,
    pub allow_http: bool,
    pub key: Secret,
    pub model: String,
    pub system: String,
    pub tools: Vec<ToolDef>,
    pub vision: bool,
    pub max_tokens: u32,
    pub max_steps: usize,
    pub context_window: u32,
    pub folders: Vec<std::path::PathBuf>,
    pub history: Vec<Message>,
    pub user: Message,
    pub host: Arc<dyn ToolHost>,
    /// Asks the window for a frame (the worker calls it after every message).
    pub repaint: Arc<dyn Fn() + Send + Sync>,
}

/// What the worker sends.
pub enum WorkerMsg {
    Event(Event),
    /// The turn is over; `history` is the conversation as the model has it now.
    Done { outcome: TurnOutcome, history: Vec<Message> },
}

/// A running turn.
pub struct TurnHandle {
    pub rx: Receiver<WorkerMsg>,
    pub cancel: Cancel,
}

/// Start `job` on its own thread.
#[cfg(not(target_arch = "wasm32"))]
pub fn spawn_turn(job: Job, cancel: Cancel) -> Result<TurnHandle, String> {
    let (tx, rx) = channel::<WorkerMsg>();
    let worker_cancel = cancel.clone();
    std::thread::Builder::new()
        .name("assistant-turn".into())
        .spawn(move || work(job, worker_cancel, tx))
        .map_err(|e| format!("cannot start the assistant: {e}"))?;
    Ok(TurnHandle { rx, cancel })
}

#[cfg(target_arch = "wasm32")]
pub fn spawn_turn(_job: Job, _cancel: Cancel) -> Result<TurnHandle, String> {
    Err("the assistant needs the desktop app".into())
}

#[cfg(not(target_arch = "wasm32"))]
fn work(job: Job, cancel: Cancel, tx: Sender<WorkerMsg>) {
    let Job { codec, http, base_url, allow_http, key, model, system, tools, vision, max_tokens, max_steps, context_window, folders, mut history, user, host, repaint } = job;
    let send = |m: WorkerMsg| {
        let _ = tx.send(m);
        repaint();
    };
    // A bug in a codec or a tool must end the turn with a message, not take the app down.
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let cfg = TurnConfig {
            codec: codec.as_ref(),
            http: &http,
            base_url: &base_url,
            allow_http,
            key: &key,
            model: &model,
            system: &system,
            tools: &tools,
            vision,
            max_tokens,
            max_steps,
            context_window,
            folders: &folders,
        };
        run_turn(&cfg, &mut history, user, host.as_ref(), &cancel, &mut |e| send(WorkerMsg::Event(e)))
    }))
    .unwrap_or_else(|_| TurnOutcome { finish: Finish::Error("The assistant hit an internal error and stopped.".into()), usage: Usage::default(), steps: 0, tool_calls: 0 });
    send(WorkerMsg::Done { outcome, history });
}

/// The folders (as strings) an assistant's tool calls may use: the app's recovery folder, the
/// private scratch folder for pictures, and `folders`. The app puts them in every request's
/// `allow_paths`.
pub fn agent_roots(folders: &[String]) -> Vec<String> {
    let scope = solidcraft_mcp::agent_scope(folders.iter().map(std::path::PathBuf::from));
    scope.roots().iter().map(|p| p.to_string_lossy().into_owned()).collect()
}
