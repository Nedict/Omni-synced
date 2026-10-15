# Agent chat

Agent chat is a side panel where you talk to a coding agent you already use (Claude Code,
Codex, Gemini CLI, OpenCode and others) and have it work on the design that's open. The agent
runs on your computer under **your own account** with that agent. SolidCraft doesn't sell or
proxy a model, doesn't ask for an API key and never stores one.

It's off by default and only in the desktop app.

## Turning it on

1. Open **Preferences → General** and tick **Agent chat**.
2. Open the panel in one of three ways:
   - **View settings** (the last button of the navigation bar under the view) → **Agent chat**;
   - the small chat button in the application bar, left of Help;
   - **Ctrl+Shift+A** (Cmd+Shift+A on a Mac). It works from the chat's own text box too.

The same preference page sets the side the panel docks on and how many changes the agent may
make for one message (24 by default).

## The panel

- **Docking:** drag the panel's header (the dotted grip and the title) to the other half of the
  window and let go. The side it would land on lights up while you drag. You can also pick
  **Left** or **Right** in Preferences. SolidCraft remembers the side.
- **Width:** drag the panel's inner edge. SolidCraft remembers it.
- **Minimise** (the line in the header) folds the panel into a thin strip that keeps the chat
  icon, the agent's state, and a count of anything waiting for your answer. Click the icon to
  unfold it. **Close** (×) hides the panel. Neither stops the agent; **Stop** does.

## Picking an agent

The panel looks for agents that speak the [Agent Client Protocol](https://agentclientprotocol.com)
on your `PATH` and in the usual install folders a desktop app's `PATH` often misses (npm's
global folder, nvm, Volta, bun, `~/.local/bin`, Homebrew, `%APPDATA%\npm`). For each one it
finds, it shows:

- its **version** (from `--version`, given three seconds);
- whether it looks **signed in**. SolidCraft only checks whether the agent's own login file
  (`~/.claude/.credentials.json`, `~/.codex/auth.json`, `~/.gemini/oauth_creds.json`, …) or one
  of its key variables exists. It never reads the file and never starts the agent to find out.
  On macOS Claude Code keeps its login in the keychain, so the panel doesn't guess there.

| Agent | What to install |
|---|---|
| Claude Code | `npm install -g @agentclientprotocol/claude-agent-acp` (the ACP adapter for Claude Code) |
| Codex | `npm install -g @agentclientprotocol/codex-acp` |
| Gemini CLI | `npm install -g @google/gemini-cli` (it speaks ACP itself: `gemini --acp`) |
| OpenCode | `npm install -g opencode-ai` (`opencode acp`) |
| GitHub Copilot CLI, Qwen Code, goose | listed with their install command when missing |

The panel lists agents that aren't installed with their install command and a **Copy** button.
**Refresh** looks again after you install one. **Add custom command…** takes any other program
that speaks ACP on stdin and stdout, such as `my-agent --acp`. SolidCraft starts it directly, not
through a shell.

## Signing in

If the agent isn't signed in, it says so and the panel shows **Sign in to the agent** with the
agent's own ways to sign in:

- **A command for a terminal**, e.g. `claude-agent-acp --cli auth login --claudeai`, with a **Copy**
  button. Run it in a terminal, sign in there, then press **I've signed in: connect again**.
- **A button** for agents that sign in by themselves (Codex's ChatGPT login opens your browser).
- **A variable to set** for agents that take a key from the environment. The panel has no field
  for a key, on purpose.

Whatever you typed before signing in is sent once the agent is ready.

## What the agent can do

The agent reaches the open design only through SolidCraft's own tools (the
[MCP server](mcp.md)), which SolidCraft hands it when the chat starts:

- **Read the design:** list commands, inspect features, sketches and parameters, measure,
  take pictures of the view.
- **Change the design:** run any modelling command, alone or in a batch. Changes show in the
  window as they happen.
- **One undo per message:** everything the agent changes while answering one message becomes
  a single undo step, named after your message. The conversation shows *N changes, one undo
  step* with an **Undo** button while nothing else has happened since.
- **Attach view:** tick it to send a picture of the current view with your message, for agents
  that take pictures.

## What it can't do, and what it asks first

- **Files wait for you.** Saving, opening, exporting or importing, starting a new design, any
  command given a file name, and undoing past the start of your message all stop at an
  **Allow / Deny** card in the panel. Nothing runs until you choose. **Cancel** or **Stop**
  declines everything that's waiting, and a question nobody answers is declined after four
  minutes. The [MCP server's folder limits](mcp.md) still apply after you allow:
  the design's folder, the recovery folder and the server's own scratch folder.
- **A limit per message.** The agent may make 24 changes per message by default; you can set
  the number in Preferences. Reading the design doesn't count. When it reaches the limit,
  it's told to stop and report what's left; send another message to let it carry on.
- **No crash reports.** The crash-report tools aren't available to the agent. Crash reports can
  hold other designs and file paths.
- **Not the rest of the app.** It can't click the interface, change preferences, quit, or
  drive the chat itself.
- **Its own tools stay off where we can turn them off.** SolidCraft declares no file system and
  no terminal to the agent, runs it in an empty working folder of its own, and asks the Claude
  Code adapter to turn off its shell, file and web tools for the session. Other agents' own
  permission questions appear in the panel with the agent's options.

When the panel closes or SolidCraft quits, the agent and every program it started (the CLI it
wraps, its MCP server) are stopped.

## Privacy

Nothing goes anywhere except to the agent you run. SolidCraft starts it as a program on your
computer and talks to it over its standard input and output. What the agent sends to its own
service, under your account and that service's terms, is between you and that service, exactly
as when you use the agent in a terminal. SolidCraft never sees, stores or passes on a password,
token or API key. Its own connection to the agent's tools uses a random per-launch token in a
file only you can read, which never passes through the agent.

## Trying it without an account

A scripted test agent is in the source tree. It makes a block, rounds it and measures it, with
no model behind it:

```sh
cargo build --release -p solidcraft-ui-egui --example acp_test_agent
```

Then choose **Add custom command…** and enter the path of `target/release/examples/acp_test_agent`.
Put "save" in your message to see an Allow / Deny card, or "ask" to see the agent ask its own
permission.
