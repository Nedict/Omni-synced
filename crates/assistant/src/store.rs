//! Saved conversations: one JSON file per conversation, grouped by design, in the chat's own data
//! folder. They are never written into a design file, so sharing a design never shares a chat.
//!
//! The store is generic over the conversation's items (JSON objects with a `kind`, see
//! [`export_markdown`]), so the panel keeps its own item types. Files are private to the user where
//! the platform lets us say so (`0600` in a `0700` folder on Unix; on Windows the per-user data
//! folder's own permissions). Loading is lenient: a damaged or hostile file is skipped, never fatal,
//! and sizes are capped.

use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// Conversations kept per design; the oldest are deleted.
pub const MAX_PER_DESIGN: usize = 50;

/// Largest saved conversation. A longer one loses its oldest items when saved.
pub const MAX_FILE: usize = 2 * 1024 * 1024;

/// Most items kept in one conversation.
pub const MAX_ITEMS: usize = 2000;

/// A conversation as saved.
#[derive(Clone, Debug, PartialEq)]
pub struct Saved {
    pub id: String,
    pub title: String,
    /// Seconds since the Unix epoch.
    pub updated: u64,
    /// The agent's name, for the listing.
    pub agent: String,
    pub items: Vec<Value>,
}

/// What a listing shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Summary {
    pub id: String,
    pub title: String,
    pub updated: u64,
    pub agent: String,
}

/// The folder name for a design: a hash of its path, so no file name leaks into a listing, and
/// `unsaved` for a design without a file.
pub fn key_for(design_path: Option<&str>) -> String {
    match design_path {
        None => "unsaved".into(),
        Some(p) => Sha256::digest(p.as_bytes()).iter().take(8).map(|b| format!("{b:02x}")).collect(),
    }
}

/// A new conversation id: the time in milliseconds, in hex, with a counter against clashes.
pub fn new_id() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    let ms = web_time::SystemTime::now().duration_since(web_time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    format!("c{ms:x}-{:x}", N.fetch_add(1, Ordering::Relaxed))
}

/// Seconds since the Unix epoch.
pub fn now() -> u64 {
    web_time::SystemTime::now().duration_since(web_time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Where conversations live.
pub struct Store {
    root: PathBuf,
}

impl Store {
    pub fn at(root: impl Into<PathBuf>) -> Store {
        Store { root: root.into() }
    }

    fn dir(&self, key: &str) -> Option<PathBuf> {
        safe_name(key).then(|| self.root.join(key))
    }

    fn file(&self, key: &str, id: &str) -> Option<PathBuf> {
        safe_name(id).then(|| self.dir(key).map(|d| d.join(format!("{id}.json")))).flatten()
    }

    /// Save a conversation, dropping its oldest items while it is over [`MAX_FILE`], then delete
    /// the oldest conversations beyond [`MAX_PER_DESIGN`].
    pub fn save(&self, key: &str, conv: &Saved) -> Result<(), String> {
        let path = self.file(key, &conv.id).ok_or("invalid conversation name")?;
        let mut items: Vec<Value> = conv.items.iter().rev().take(MAX_ITEMS).rev().cloned().collect();
        let text = loop {
            let doc = json!({"version": 1, "title": conv.title, "updated": conv.updated, "agent": conv.agent, "items": items});
            let t = serde_json::to_vec(&doc).map_err(|e| e.to_string())?;
            if t.len() <= MAX_FILE {
                break t;
            }
            if items.len() <= 1 {
                return Err("the conversation is too large to save".into());
            }
            let drop = (items.len() / 10).max(1);
            items.drain(..drop);
        };
        write_private(&path, &text).map_err(|e| format!("cannot save the conversation: {e}"))?;
        self.prune(key);
        Ok(())
    }

    fn prune(&self, key: &str) {
        let mut all = self.list(key);
        if all.len() <= MAX_PER_DESIGN {
            return;
        }
        all.sort_by_key(|s| std::cmp::Reverse(s.updated));
        for old in all.into_iter().skip(MAX_PER_DESIGN) {
            self.delete(key, &old.id);
        }
    }

    /// The conversations of `key`, newest first.
    pub fn list(&self, key: &str) -> Vec<Summary> {
        let Some(dir) = self.dir(key) else { return Vec::new() };
        let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
        let mut out: Vec<Summary> = rd
            .flatten()
            .take(1000)
            .filter_map(|e| {
                let p = e.path();
                let id = p.file_stem()?.to_str()?.to_string();
                if p.extension()? != "json" || !safe_name(&id) {
                    return None;
                }
                let v = read_json(&p)?;
                Some(Summary {
                    id,
                    title: short(v.get("title")),
                    updated: v.get("updated").and_then(Value::as_u64).unwrap_or(0),
                    agent: short(v.get("agent")),
                })
            })
            .collect();
        out.sort_by_key(|s| std::cmp::Reverse(s.updated));
        out
    }

    pub fn load(&self, key: &str, id: &str) -> Option<Saved> {
        let v = read_json(&self.file(key, id)?)?;
        let items: Vec<Value> = v.get("items")?.as_array()?.iter().filter(|i| i.is_object()).take(MAX_ITEMS).cloned().collect();
        Some(Saved {
            id: id.to_string(),
            title: short(v.get("title")),
            updated: v.get("updated").and_then(Value::as_u64).unwrap_or(0),
            agent: short(v.get("agent")),
            items,
        })
    }

    pub fn delete(&self, key: &str, id: &str) {
        if let Some(p) = self.file(key, id) {
            let _ = std::fs::remove_file(p);
        }
    }
}

fn short(v: Option<&Value>) -> String {
    v.and_then(Value::as_str).unwrap_or("").chars().take(120).collect()
}

/// A file or folder name made of letters, digits, `-` and `_`: nothing that could leave the
/// store's folder.
fn safe_name(s: &str) -> bool {
    !s.is_empty() && s.len() <= 64 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn read_json(path: &Path) -> Option<Value> {
    let meta = std::fs::metadata(path).ok()?;
    if meta.len() > (MAX_FILE as u64) * 2 {
        return None;
    }
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

/// Write `bytes` to `path` through a sibling file, readable by the user alone on Unix.
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let dir = path.parent().ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "no folder"))?;
    let fresh = !dir.exists();
    std::fs::create_dir_all(dir)?;
    #[cfg(unix)]
    if fresh {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }
    #[cfg(not(unix))]
    let _ = fresh;
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = dir.join(format!(".{name}.{}.tmp", std::process::id()));
    let _ = std::fs::remove_file(&tmp);
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let result = (|| {
        let mut f = opts.open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        std::fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// A conversation as Markdown, for export. Items are JSON objects with a `kind`: `user` and
/// `agent` (`text`; `thought: true` for reasoning, which is left out), `tool` (`title`,
/// `status`), `plan` (`entries`: `[content, status]`), `notice` (`text`). Other kinds are
/// ignored.
pub fn export_markdown(items: &[Value]) -> String {
    let text = |i: &Value, k: &str| i.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let mut out = String::from("# SolidCraft agent chat\n\n");
    for i in items {
        match i.get("kind").and_then(Value::as_str) {
            Some("user") => out.push_str(&format!("## You\n\n{}\n\n", text(i, "text"))),
            Some("agent") if i.get("thought").and_then(Value::as_bool) != Some(true) => out.push_str(&format!("## Agent\n\n{}\n\n", text(i, "text"))),
            Some("tool") => out.push_str(&format!("> `{}` ({})\n\n", text(i, "title"), text(i, "status"))),
            Some("plan") => {
                for e in i.get("entries").and_then(Value::as_array).into_iter().flatten().take(50) {
                    let pair = e.as_array();
                    let (c, s) = (
                        pair.and_then(|p| p.first()).and_then(Value::as_str).unwrap_or(""),
                        pair.and_then(|p| p.get(1)).and_then(Value::as_str).unwrap_or(""),
                    );
                    out.push_str(&format!("- [{}] {c}\n", if s == "completed" { "x" } else { " " }));
                }
                out.push('\n');
            }
            Some("notice") => out.push_str(&format!("*{}*\n\n", text(i, "text"))),
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("solidcraft-chat-store-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn conv(id: &str, updated: u64, n: usize) -> Saved {
        let mut items = Vec::new();
        for i in 0..n {
            items.push(json!({"kind": "user", "text": format!("question {i}")}));
            items.push(json!({"kind": "tool", "title": "batch", "status": "completed"}));
            items.push(json!({"kind": "agent", "text": format!("answer {i}")}));
        }
        Saved { id: id.into(), title: "question 0".into(), updated, agent: "Claude".into(), items }
    }

    #[test]
    fn a_conversation_round_trips() {
        let s = Store::at(dir("roundtrip"));
        let c = conv("c1", 100, 2);
        s.save("abc", &c).unwrap();
        assert_eq!(s.load("abc", "c1").unwrap(), c);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let p = s.file("abc", "c1").unwrap();
            assert_eq!(std::fs::metadata(&p).unwrap().permissions().mode() & 0o777, 0o600);
            assert_eq!(std::fs::metadata(p.parent().unwrap()).unwrap().permissions().mode() & 0o777, 0o700);
        }
        // Saving again replaces it, and leaves no temporary file.
        s.save("abc", &conv("c1", 200, 3)).unwrap();
        assert_eq!(s.load("abc", "c1").unwrap().items.len(), 9);
        assert_eq!(std::fs::read_dir(s.dir("abc").unwrap()).unwrap().count(), 1);
    }

    #[test]
    fn listing_is_newest_first_and_skips_damaged_files() {
        let root = dir("list");
        let s = Store::at(&root);
        s.save("k", &conv("a", 10, 1)).unwrap();
        s.save("k", &conv("b", 30, 1)).unwrap();
        s.save("k", &conv("c", 20, 1)).unwrap();
        std::fs::write(root.join("k/bad.json"), "{ nope").unwrap();
        std::fs::write(root.join("k/notes.txt"), "x").unwrap();
        std::fs::write(root.join("k/we ird.json"), "{}").unwrap();
        let ids: Vec<String> = s.list("k").into_iter().map(|x| x.id).collect();
        assert_eq!(&ids[..3], ["b", "c", "a"]);
        assert!(s.load("k", "bad").is_none());
        assert!(s.list("missing").is_empty());
    }

    #[test]
    fn names_cannot_leave_the_store() {
        let s = Store::at(dir("names"));
        for bad in ["../x", "a/b", "a\\b", "", "..", ".hidden", &"a".repeat(65), "a b"] {
            assert!(s.save("k", &conv(bad, 1, 1)).is_err(), "{bad:?}");
            assert!(s.load("k", bad).is_none());
            assert!(s.list(bad).is_empty());
        }
        assert_eq!(key_for(None), "unsaved");
        assert_eq!(key_for(Some("/a/b.solidcraft")).len(), 16);
        assert_ne!(key_for(Some("/a")), key_for(Some("/b")));
        assert!(safe_name(&key_for(Some("../../etc"))));
    }

    #[test]
    fn only_the_newest_are_kept() {
        let s = Store::at(dir("prune"));
        for i in 0..(MAX_PER_DESIGN + 5) {
            s.save("k", &conv(&format!("c{i}"), i as u64, 1)).unwrap();
        }
        let all = s.list("k");
        assert_eq!(all.len(), MAX_PER_DESIGN);
        assert!(all.iter().all(|x| x.updated >= 5));
    }

    #[test]
    fn a_huge_conversation_loses_its_oldest_items() {
        let s = Store::at(dir("big"));
        let items: Vec<Value> = (0..40).map(|i| json!({"kind": "agent", "text": format!("{i:02} {}", "x".repeat(100_000))})).collect();
        s.save("k", &Saved { id: "big".into(), title: "t".into(), updated: 1, agent: "a".into(), items }).unwrap();
        let back = s.load("k", "big").unwrap();
        assert!(back.items.len() < 40 && !back.items.is_empty());
        assert!(back.items.last().and_then(|i| i["text"].as_str()).is_some_and(|t| t.starts_with("39 ")), "the newest item stays");
    }

    #[test]
    fn hostile_files_load_as_nothing_or_less() {
        let root = dir("hostile");
        let s = Store::at(&root);
        std::fs::create_dir_all(root.join("k")).unwrap();
        for (name, body) in [
            ("a", r#"{"items": "x"}"#),
            ("b", r#"{"items": [1, null, {"kind": "user"}, "s"]}"#),
            ("c", r#"[]"#),
            ("d", r#"{"title": 5, "items": []}"#),
        ] {
            std::fs::write(root.join(format!("k/{name}.json")), body).unwrap();
        }
        assert!(s.load("k", "a").is_none());
        assert_eq!(s.load("k", "b").unwrap().items.len(), 1);
        assert!(s.load("k", "c").is_none());
        assert_eq!(s.load("k", "d").unwrap().title, "");
    }

    #[test]
    fn export_reads_like_a_chat_and_leaves_thoughts_out() {
        let mut c = conv("x", 1, 1);
        c.items.push(json!({"kind": "agent", "text": "private reasoning", "thought": true}));
        c.items.push(json!({"kind": "plan", "entries": [["Make box", "completed"], ["Fillet", "pending"]]}));
        c.items.push(json!({"kind": "notice", "text": "Agent stopped"}));
        c.items.push(json!({"kind": "approval"}));
        let md = export_markdown(&c.items);
        assert!(md.contains("## You\n\nquestion 0"));
        assert!(md.contains("> `batch` (completed)"));
        assert!(md.contains("## Agent\n\nanswer 0"));
        assert!(md.contains("- [x] Make box") && md.contains("- [ ] Fillet"));
        assert!(md.contains("*Agent stopped*"));
        assert!(!md.contains("private reasoning"));
    }
}
