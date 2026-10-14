//! The agent as a child process (desktop only): its stdout is read on a thread and handed to the
//! panel line by line, its stdin written on another so a slow agent never blocks a frame. The
//! child is ours alone, so it is stopped by its own handle (never by name).

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, Sender, channel};

/// Longest line read from the agent; a longer one is dropped.
const MAX_LINE: usize = 8 << 20;
/// Lines of the agent's stderr kept to explain a failed start.
const STDERR_LINES: usize = 40;

pub enum FromAgent {
    Line(String),
    /// stdout closed: the agent ended. Its last stderr lines, for the panel.
    Exited(Vec<String>),
}

pub struct AgentProcess {
    child: Child,
    to_agent: Sender<String>,
    pub from_agent: Receiver<FromAgent>,
}

/// On Windows, npm installs `.cmd` shims, which only `cmd.exe` runs.
fn command_for(program: &str) -> Command {
    let lower = program.to_ascii_lowercase();
    if cfg!(windows) && (lower.ends_with(".cmd") || lower.ends_with(".bat")) {
        let mut c = Command::new("cmd");
        c.arg("/C").arg(program);
        c
    } else {
        Command::new(program)
    }
}

impl AgentProcess {
    /// Start `argv` (program first) in `cwd`; `wake` is called whenever a line arrives.
    pub fn spawn(argv: &[String], cwd: &std::path::Path, wake: impl Fn() + Send + Sync + 'static) -> Result<AgentProcess, String> {
        let Some((program, args)) = argv.split_first() else { return Err("no agent command".into()) };
        let mut cmd = command_for(program);
        cmd.args(args).current_dir(cwd).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = cmd.spawn().map_err(|e| format!("{program}: {e}"))?;
        let (Some(mut stdin), Some(stdout), Some(stderr)) = (child.stdin.take(), child.stdout.take(), child.stderr.take()) else {
            let _ = child.kill();
            return Err("the agent's pipes could not be opened".into());
        };
        let (to_agent, rx) = channel::<String>();
        std::thread::spawn(move || {
            for line in rx {
                if stdin.write_all(line.as_bytes()).and_then(|()| stdin.write_all(b"\n")).and_then(|()| stdin.flush()).is_err() {
                    break;
                }
            }
        });
        let tail = std::sync::Arc::new(std::sync::Mutex::new(std::collections::VecDeque::<String>::new()));
        let tail_w = tail.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                if let Ok(mut t) = tail_w.lock() {
                    if t.len() >= STDERR_LINES {
                        t.pop_front();
                    }
                    t.push_back(line.chars().take(400).collect());
                }
            }
        });
        let (tx, from_agent) = channel::<FromAgent>();
        let wake = std::sync::Arc::new(wake);
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut buf = Vec::new();
                match (&mut reader).take(MAX_LINE as u64 + 1).read_until(b'\n', &mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(_) if buf.len() > MAX_LINE => {
                        // Skip the rest of an oversized line.
                        let mut rest = Vec::new();
                        if reader.read_until(b'\n', &mut rest).is_err() {
                            break;
                        }
                    }
                    Ok(_) => {
                        if tx.send(FromAgent::Line(String::from_utf8_lossy(&buf).into_owned())).is_err() {
                            return;
                        }
                        wake();
                    }
                }
            }
            // Give stderr a moment to deliver the agent's last words.
            std::thread::sleep(std::time::Duration::from_millis(100));
            let lines = tail.lock().map(|t| t.iter().cloned().collect()).unwrap_or_default();
            let _ = tx.send(FromAgent::Exited(lines));
            wake();
        });
        Ok(AgentProcess { child, to_agent, from_agent })
    }

    pub fn send(&self, msg: &serde_json::Value) {
        let _ = self.to_agent.send(msg.to_string());
    }

    pub fn pid(&self) -> u32 {
        self.child.id()
    }
}

impl Drop for AgentProcess {
    fn drop(&mut self) {
        // Ours alone: stopped through its own handle, then reaped.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
