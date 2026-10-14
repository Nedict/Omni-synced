# In-app agent chat over ACP (prototype)

Status: prototype behind a preference (off by default), desktop only. This note records what I
found about the Agent Client Protocol and the agents that speak it, how the prototype works, and
what a production version still needs.

## Goal

A chat panel inside SolidCraft that drives a coding agent **the user already has and is already
signed in to** (Claude Code, Codex, Gemini CLI, …). SolidCraft holds no API keys and no
accounts: the agent runs as a child process under the user's own login, and SolidCraft gives
it one thing, its MCP server bridged to the open window, so the agent models on the design the
user is looking at.

## The protocol

[ACP](https://agentclientprotocol.com) is JSON-RPC 2.0 over the agent's stdin/stdout, one
message per line, protocol version `1`. The editor (here SolidCraft) is the *client*; the agent
is the *server*, started as a subprocess. Zed introduced it to embed Claude Code and Codex;
the Rust SDK (`agent-client-protocol`, Apache-2.0) and the schema crate
(`agent-client-protocol-schema`, Apache-2.0) are the reference.

What the prototype uses:

| Direction | Message | Use |
|---|---|---|
| → agent | `initialize {protocolVersion: 1, clientCapabilities, clientInfo}` | We declare no file system and no terminal (`fs.readTextFile/writeTextFile: false`, `terminal: false`), and terminal login (`auth.terminal: true`, plus the older `_meta["terminal-auth"]`). The reply carries `agentInfo` and `authMethods`. |
| → agent | `session/new {cwd, mcpServers}` | `mcpServers` holds one stdio server: `solidcraft-cli mcp --connect 127.0.0.1:PORT`. `cwd` is the open design's folder (else SolidCraft's own `agent` folder). |
| → agent | `session/prompt {sessionId, prompt: [content]}` | One text block. The reply comes at the end of the turn with `stopReason` (`end_turn`, `cancelled`, `refusal`, …). |
| → agent | `session/cancel {sessionId}` (notification) | Cancel button. Open permission prompts are answered `cancelled` first, as the spec asks. |
| → agent | `authenticate {methodId}` | Only for an "agent" login method the user clicks (the agent then opens the browser itself). |
| ← agent | `session/update` | `agent_message_chunk`, `agent_thought_chunk` (streamed text), `tool_call` / `tool_call_update` (`toolCallId`, `title`, `kind`, `status`: pending, in_progress, completed, failed, `content`), `plan` (entries with status). Mode, command and usage updates are ignored for now. |
| ← agent | `session/request_permission {toolCall, options: [{optionId, name, kind}]}` | Shown as a prompt in the conversation; the answer is `{outcome: {outcome: "selected", optionId}}` or `{outcome: "cancelled"}`. `kind` is `allow_once`, `allow_always`, `reject_once` or `reject_always`. |
| ← agent | `fs/*`, `terminal/*` | Not offered, answered "method not found". |
| ← agent | `_auth/status_update` | An extension both current adapters send ("Not logged in"); shown next to the agent's name. |

Errors: an agent that is not signed in answers `session/new` (Codex) or the first
`session/prompt` (Claude) with JSON-RPC error `-32000 "Authentication required"`.

## The agents

All checked on 2026-10-14 with the packages installed into a user prefix
(`npm i -g --prefix ~/…`). Licences are from each package's own metadata; nothing is bundled
with SolidCraft, the user installs the agent.

| Agent | How it speaks ACP | Package / licence | Version probe | Login |
|---|---|---|---|---|
| Claude Code | `claude-agent-acp` adapter (renamed from `@zed-industries/claude-code-acp`, which is now deprecated) | `@agentclientprotocol/claude-agent-acp` 0.90.0, Apache-2.0. It depends on `@anthropic-ai/claude-agent-sdk`, which is under Anthropic's own terms ("SEE LICENSE IN README.md"): fine for the user to install, not something we would vendor. | `--version` prints `0.90.0` | Terminal method. Offered only when the client declares terminal login. On a desktop: "Claude Subscription" = `claude-agent-acp --cli auth login --claudeai`, "Anthropic Console" = `… --cli auth login --console`. Over SSH (no browser) it offers `claude-login` = `… --cli` (the TUI `/login`). The login is Claude Code's own: `~/.claude/.credentials.json` (Linux, Windows) or the macOS keychain. |
| Codex | `codex-acp` adapter (`@zed-industries/codex-acp` 0.16 moved to `@agentclientprotocol/codex-acp`) | `@agentclientprotocol/codex-acp` 2.2.2, Apache-2.0 | `--version` (0.16 rejected the flag; 2.x prints it) | Agent method `chat-gpt` (the adapter opens the browser for the ChatGPT login when the client calls `authenticate`), and `api-key` for a key the user configures in Codex. 0.16 also listed `env_var` methods for `CODEX_API_KEY` / `OPENAI_API_KEY`. The login is Codex's own: `~/.codex/auth.json`. |
| Gemini CLI | built in: `gemini --acp` (the ACP registry's current flag; older releases used `--experimental-acp`) | `@google/gemini-cli` 0.64, Apache-2.0 | `--version` | Its own Google login (`~/.gemini/oauth_creds.json`) or a key in the environment. |
| Others in the ACP registry | `copilot --acp`, `qwen --acp`, `opencode acp`, `goose acp`, `cline --acp`, `kimi acp`, … | mixed; several are proprietary, one is GPL and one AGPL (we only start them as programs, never link them) | `--version` | each its own |

The registry (`https://cdn.agentclientprotocol.com/registry/v1/latest/registry.json`) lists
about forty agents with their `npx`/binary distribution; a later version could read it to fill
the picker instead of the hard-coded table.

### The auth story

- **SolidCraft never sees a credential.** It doesn't pass API keys, doesn't store tokens and
  doesn't open login pages by itself. Each agent keeps its own login where its CLI always does
  (Claude Code: `~/.claude` or the keychain; Codex: `~/.codex`; Gemini: `~/.gemini`). A user
  who already uses the agent in a terminal is already signed in, and the chat simply works.
- **Not signed in:** the agent answers "Authentication required". The panel switches to
  *Sign-in needed* and lists the agent's own `authMethods`:
  - *terminal* methods are shown as the exact command to run in a terminal (with Copy), e.g.
    `claude-agent-acp --cli auth login --claudeai`; the user runs it, then presses Connect;
  - *agent* methods (Codex's ChatGPT login) get a button that calls `authenticate`; the agent
    opens the browser itself;
  - *env_var* methods say which variable to set in the user's environment. The panel has no
    field for a key, on purpose.
- Verified here, against both adapters, with an empty home folder (no credentials, so nobody's
  account was used): Claude's adapter returns `-32000 Authentication required` on the first
  prompt and offers the terminal login methods; Codex's returns it on `session/new` and offers
  `chat-gpt` and `api-key`. I stopped at that step.

## How the prototype works

```
SolidCraft window ──(child process, ACP over stdio)──> agent (claude-agent-acp, codex-acp, gemini --acp, …)
      ▲                                                   │ starts, per session/new
      │ control channel (loopback, token, #205)           ▼
      └────────────────────────────────── solidcraft-cli mcp --connect 127.0.0.1:PORT
```

- **Panel** (`crates/ui-egui/src/agent_chat/`): a resizable right-hand side panel, toggled from
  the application bar's chat button when *Preferences → General → Agent chat* is on. Header with
  state and Connect/Stop; an agent picker; the conversation (streamed text, thoughts dimmed,
  tool calls with status and output, the plan, permission prompts with the agent's own option
  buttons, notices); a prompt box (Enter sends, Shift+Enter is a new line) and Cancel.
- **Agent discovery** (`discover.rs`): looks for the known agents on `PATH` and in the usual
  install folders a desktop app's PATH misses (npm prefix from `NPM_CONFIG_PREFIX`,
  `~/.npm-global/bin`, every `~/.nvm/versions/node/*/bin`, `~/.local/bin`, `~/.volta/bin`,
  `~/.bun/bin`, Homebrew, `%APPDATA%\npm` on Windows). Each one found is asked `--version` with a
  3 s limit (stopped through its own handle if it overruns); whether it *looks* signed in comes
  only from whether its own credentials file exists, never from starting it. The picker shows
  name, version and signed-in state; *Add custom command…* takes any other ACP command; agents
  not found are listed with their one-line install command. The result is cached; Refresh
  looks again. The whole sweep runs on a background thread.
- **Preferences:** `agent_chat_enabled` (default off: no panel, no button) and `agent_command`
  (empty: the first agent found, the Claude Code adapter first). On the web build the
  checkbox is disabled with a note.
- **Design tools:** on Connect the app starts its control server on a free loopback port if it
  wasn't launched with `--control` (`control_server::listen(0, …)`), and hands the agent
  `solidcraft-cli mcp --connect 127.0.0.1:PORT`. Everything #205 added stays in force:
  - the control port still needs the launch's token; the bridged MCP server reads it from the
    per-user token file, so the token never travels through the agent's messages;
  - the MCP server keeps its default folder limits (the design's folder when it is a safe one,
    the recovery folder, its own scratch folder); `--allow-any-path` is never passed, and the
    app checks every bridged request against the folders it carries;
  - the tool annotations are unchanged, so an agent that asks before destructive tools asks.
- **Permissions:** the agent's own permission requests appear in the conversation; nothing is
  auto-approved. For the Claude adapter the session also sets
  `_meta.claudeCode.options.allowDangerouslySkipPermissions: false`, so its "bypass
  permissions" mode isn't offered. A short system note (`_meta.systemPrompt.append`) tells the
  agent it is inside SolidCraft and which MCP tools to use.
- **Control channel:** `ui.agent_chat` (docs/control-protocol.md) opens the panel, sets the
  preference and command, connects, sends a prompt, answers a permission request and returns the
  whole conversation, for tests and screenshots.

## What works (checked end to end)

With a scripted ACP agent (a small test script that speaks ACP like the adapters and starts the
MCP server it is given) against the release app under Xvfb:

1. The picker found `claude-agent-acp 0.90.0` and `codex-acp 2.2.2` (installed into a user
   prefix on PATH) and the `gemini`/`copilot`/… install hints for the rest.
2. Connect started the control server on a free port and the agent; the agent's
   `session/new` carried `solidcraft-cli mcp --connect 127.0.0.1:PORT`, and the MCP server
   listed SolidCraft's tools.
3. A prompt streamed text and a plan, showed a tool call, and asked permission; *Allow* ran
   `batch [solid.box 40×30×20]` through the bridge, the box appeared in the window and the
   browser, and `measure` reported its 40 × 30 × 20 mm bounding box.
4. A `save` to `/etc/…` was refused by the #205 folder check and shown as a failed tool call.
5. Against the real Claude and Codex adapters with no credentials, the panel reached
   *Sign-in needed* and listed the adapters' own login methods.

## What's missing for production

| Area | Work | Estimate |
|---|---|---|
| Conversation | Markdown rendering (code blocks, lists), images, diffs from `tool_call` content, copy buttons, history kept per design | 3–4 days |
| Session controls | Model and mode pickers from `configOptions` / `modes`, slash commands from `available_commands_update`, resume (`session/load`), several sessions | 2–3 days |
| Auth | An embedded terminal or "open a terminal here" for terminal logins on each OS, status refresh after login, logout | 2 days |
| Process handling | Windows `.cmd` shims tested, killing the agent's whole process tree (Job object / process group), restart on crash, stderr log view | 2 days |
| Discovery | Read the ACP registry, `npm prefix -g` with a timeout, keychain-aware login check on macOS, per-agent args editor | 1–2 days |
| Safety | Decide what the agent's *own* tools may do: Claude Code also has shell and file tools that act on `cwd`; options are to run in an empty working folder, disallow its shell/edit tools via `_meta.claudeCode.options.disallowedTools`, or show them with an extra warning. Per-tool "always allow" kept per design. | 2 days + a decision |
| Packaging | Make sure every desktop package ships `solidcraft-cli` next to `solidcraft` (Flatpak, AppImage, macOS bundle, Windows zip), or serve MCP from the app binary itself | 1–2 days |
| UX polish | Dock left/right, remember width, keyboard focus, agent cursor while the agent works, i18n review | 2 days |
| Tests | UI scenario for the panel with the scripted agent in CI | 1 day |

Roughly **3 weeks** for one person to a version I'd ship on by default, most of it in auth
ergonomics, process handling across three OSes and the safety decision above.

## Russian strings

Russian is not registered on `main` yet (it is in its own pull request); these are the panel's
messages for `assets/locales/ru.json` once it lands:

```json
{
  "ui.agent_chat": "Чат с агентом",
  "ui.agent_chat_enable": "Показывать панель чата с агентом (использует ACP-агент, установленный на этом компьютере)",
  "ui.agent_chat_command": "Команда агента",
  "ui.agent_chat_command_hint": "Пусто: первый найденный агент (сначала адаптер Claude Code)",
  "ui.agent_chat_desktop_only": "Чату с агентом нужно настольное приложение.",
  "ui.agent_chat_connect": "Подключить",
  "ui.agent_chat_stop": "Остановить",
  "ui.agent_chat_send": "Отправить",
  "ui.agent_chat_cancel": "Отмена",
  "ui.agent_chat_refresh": "Обновить",
  "ui.agent_chat_add_custom": "Добавить свою команду…",
  "ui.agent_chat_custom": "Своя команда",
  "ui.agent_chat_not_installed": "Не установлен",
  "ui.agent_chat_none_found": "ACP-агент не найден. Установите один из них:",
  "ui.agent_chat_detecting": "Поиск агентов…",
  "ui.agent_chat_signed_in": "вход выполнен",
  "ui.agent_chat_not_signed_in": "вход не выполнен",
  "ui.agent_chat_state_off": "Не подключено",
  "ui.agent_chat_state_starting": "Запуск…",
  "ui.agent_chat_state_ready": "Готов",
  "ui.agent_chat_state_busy": "Работает…",
  "ui.agent_chat_state_auth": "Нужен вход",
  "ui.agent_chat_state_exited": "Агент остановлен",
  "ui.agent_chat_auth_needed": "Агент просит вас войти. SolidCraft не видит ваш пароль и ключи: агент сам хранит свой вход.",
  "ui.agent_chat_auth_terminal": "Выполните это в терминале, затем подключитесь снова:",
  "ui.agent_chat_auth_agent": "Войти через {name}",
  "ui.agent_chat_auth_env": "Задайте {vars} в своём окружении и перезапустите SolidCraft.",
  "ui.agent_chat_copy": "Копировать",
  "ui.agent_chat_permission": "Агент просит разрешения выполнить: {title}",
  "ui.agent_chat_answered": "Ответ: {option}",
  "ui.agent_chat_placeholder": "Попросите агента изменить модель…",
  "ui.agent_chat_no_cli": "solidcraft-cli не найден рядом с приложением, поэтому агент не может работать с моделью.",
  "ui.agent_chat_no_control": "Канал управления не запустился, поэтому агент не может работать с моделью.",
  "ui.agent_chat_tools": "Инструменты модели: MCP-сервер SolidCraft на порту {port}",
  "ui.agent_chat_exited": "Агент остановлен.",
  "ui.agent_chat_failed_start": "Не удалось запустить агента: {error}",
  "ui.agent_chat_intro": "Общайтесь с агентом для программирования, которым вы уже пользуетесь. Он работает с открытой моделью через инструменты SolidCraft и спрашивает разрешения, прежде чем действовать."
}
```
