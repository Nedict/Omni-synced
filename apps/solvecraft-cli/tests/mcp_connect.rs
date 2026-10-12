//! `mcp --connect` to a port where no app listens tells the user how to start one (#270).

use std::net::TcpListener;
use std::process::{Command, Stdio};

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout};

use serde_json::{Value, json};

#[test]
fn mcp_connect_without_a_listening_app_names_the_control_flag() {
    // A port that was free a moment ago: nothing listens on it.
    let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let output = Command::new(env!("CARGO_BIN_EXE_solvecraft-cli"))
        .args(["mcp", "--connect", &format!("127.0.0.1:{port}")])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    assert!(stderr.contains("--control"), "say how to start the app with a control port: {stderr}");
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

fn spawn_bridge(port: u16) -> (Child, ChildStdin, BufReader<ChildStdout>) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_solvecraft-cli"))
        .args(["mcp", "--connect", &format!("127.0.0.1:{port}")])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let stdin = child.stdin.take().unwrap();
    let stdout = BufReader::new(child.stdout.take().unwrap());
    (child, stdin, stdout)
}

fn request(stdin: &mut ChildStdin, stdout: &mut BufReader<ChildStdout>, msg: Value) -> Value {
    writeln!(stdin, "{msg}").unwrap();
    stdin.flush().unwrap();
    let mut line = String::new();
    stdout.read_line(&mut line).unwrap();
    assert!(!line.is_empty(), "the MCP server closed stdout");
    serde_json::from_str(&line).unwrap()
}

fn initialize() -> Value {
    json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "test", "version": "0"}}})
}

fn list_commands(id: u64) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": "tools/call", "params": {"name": "list_commands", "arguments": {}}})
}

fn tool_text(reply: &Value) -> &str {
    reply["result"]["content"][0]["text"].as_str().unwrap()
}

#[test]
fn mcp_connect_without_app_answers_and_names_the_control_port() {
    let port = free_port();
    let (child, mut stdin, mut stdout) = spawn_bridge(port);

    let init = request(&mut stdin, &mut stdout, initialize());
    assert_eq!(init["result"]["serverInfo"]["name"], "solvecraft", "{init}");

    let reply = request(&mut stdin, &mut stdout, list_commands(2));
    assert_eq!(reply["result"]["isError"], true, "{reply}");
    let text = tool_text(&reply);
    assert!(text.contains("--control") && text.contains(&port.to_string()), "{text}");
    assert!(text.contains("SOLVECRAFT_CONTROL_PORT"), "{text}");

    // The app never came up: the session fails, with the hint once at start and once at the end.
    drop(stdin);
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success(), "status: {}", output.status);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(stderr.matches("--control").count(), 2, "{stderr}");
    assert!(stderr.contains(&format!("--control {port}")), "{stderr}");
}

#[test]
fn mcp_connect_reaches_an_app_that_starts_later() {
    let port = free_port();
    let (mut child, mut stdin, mut stdout) = spawn_bridge(port);
    request(&mut stdin, &mut stdout, initialize());
    let before = request(&mut stdin, &mut stdout, list_commands(2));
    assert_eq!(before["result"]["isError"], true, "{before}");

    // A stand-in for the app's control channel: one connection, one reply per request line.
    let listener = TcpListener::bind(("127.0.0.1", port)).unwrap();
    let app = std::thread::spawn(move || {
        let (conn, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(conn.try_clone().unwrap());
        let mut writer = conn;
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let req: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(req["method"], "engine.commands");
        writeln!(writer, "{}", json!({"id": req["id"], "ok": true, "result": [{"id": "solid.box", "label": "Box"}]})).unwrap();
    });

    let after = request(&mut stdin, &mut stdout, list_commands(3));
    assert!(after["result"].get("isError").is_none(), "{after}");
    assert!(tool_text(&after).contains("solid.box"), "{after}");

    app.join().unwrap();
    drop(stdin);
    assert!(child.wait().unwrap().success());
}

#[test]
fn mcp_connect_to_an_address_without_a_port_fails_before_serving() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_solvecraft-cli"))
        .args(["mcp", "--connect", "127.0.0.1"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // Offer an initialize request; a server that started would answer it on stdout.
    let mut stdin = child.stdin.take().unwrap();
    writeln!(stdin, "{}", initialize()).ok();
    drop(stdin);
    let output = child.wait_with_output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    assert!(output.stdout.is_empty(), "served: {}", String::from_utf8_lossy(&output.stdout));
    assert!(stderr.contains("127.0.0.1"), "{stderr}");
}
