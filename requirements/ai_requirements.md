# AI (M5): what blackglass needs

A plan for milestone M5, **AI in blackglass**, from two Obsidian plugins
(read online 2026-09-30):

- **[Claudian](https://github.com/YishenTu/claudian)** (YishenTu, MIT):
  embeds a coding agent (Claude Code, and since 2.1.4 Codex CLI, Grok
  Build, OpenCode, Pi) with the vault as its working folder: a sidebar
  chat, inline edits with a diff, slash commands and skills, `@` mentions,
  MCP servers, tool approval and modes.
- **[Copilot for Obsidian](https://github.com/logancyang/obsidian-copilot)**
  (logancyang): chat with the notes: Quick Chat, Agent Chat (opencode,
  Claude Code, Codex), Quick Ask on a selection, custom commands with
  variables, vault search (lexical and semantic), relevant notes,
  projects, many model providers (hosted, bring your own key, local).

Each piece has an ID (`AI-xx`), where it comes from (**Cl** Claudian,
**Co** Copilot), a rough effort (S / M / L) and notes.

**Now:** ✅ done · 🟡 partial · ⬜ missing · ✗ not planned (with the reason).

It is one blackglass plugin, **AI** (id `ai`, code in `src/plugins/ai/`,
settings in `.blackglass/plugins/ai/settings.toml`), off until installed.
**Secrets never go in the vault:** API keys come from the environment
(`ANTHROPIC_API_KEY`, `OPENAI_API_KEY` …) or `~/.config/blackglass/`
(file mode 600), never from a note or `.blackglass/`. Nothing is sent
anywhere until the user asks (a chat message, a command); what a request
sends (which notes) is shown. Network and agent processes run in the
background (threads), never blocking typing; answers stream in.

## 1. Connecting to a model

| ID | Feature | From | Now | Effort | Notes |
|----|---------|------|-----|--------|-------|
| AI-01 | Providers: Anthropic (Claude) by default, OpenAI-compatible APIs (OpenAI, OpenRouter, LM Studio …), local Ollama | Co | ⬜ | M | HTTP with streaming (server-sent events); a pure-Rust HTTP / TLS client (e.g. `ureq` with `rustls`) |
| AI-02 | Model choice per chat and a default (the latest Claude models by default: `claude-opus-5-5`, `claude-sonnet-5`, `claude-haiku-4-5-20251001`), reasoning effort / thinking budget where the model has it | Cl, Co | ⬜ | S | |
| AI-03 | API keys from the environment or the user's config folder; a "test connection" row in the settings | Co | ⬜ | S | Never in the vault |
| AI-04 | Agents: run a coding agent CLI with the vault as its folder: Claude Code (`claude`, streamed JSON output), and the others Claudian and Copilot support (Codex, opencode …) through the same runner | Cl, Co | ⬜ | L | Found on `PATH` (a setting for the path and extra environment variables); desktop only upstream too |
| AI-05 | MCP servers: the agent's own MCP configuration is used (the CLI manages it) | Cl | ⬜ | S | Nothing to build beyond passing the agent's config through |

## 2. The chat

| ID | Feature | From | Now | Effort | Notes |
|----|---------|------|-----|--------|-------|
| AI-10 | A chat pane: messages rendered as Markdown (mdedit's view mode), streaming as they come, an input line (multi-line with Alt+Enter), Esc stops the answer | Cl, Co | ⬜ | L | A pane on the right that shares the screen with the notes, or a tab; opened by a command and a free key |
| AI-11 | Quick Chat: a plain conversation with the model (no tools) | Co | ⬜ | M | |
| AI-12 | Agent Chat: the agent reads and edits notes, searches, runs commands, over many steps, each tool call shown | Cl, Co | ⬜ | L | With AI-04 |
| AI-13 | Several chats in tabs; a side chat (`/side`, `/btw`) that leaves the main one unchanged | Cl, Co | ⬜ | M | |
| AI-14 | History: chats saved (per vault, in `.blackglass/plugins/ai/chats/`), reopened, renamed, deleted; a chat can be saved as a note | Cl, Co | ⬜ | M | |
| AI-15 | Copy a message, insert it at the cursor in the note, retry, edit the last message | Co | ⬜ | S | |

## 3. Context: what the model sees

| ID | Feature | From | Now | Effort | Notes |
|----|---------|------|-----|--------|-------|
| AI-20 | The active note and the selection, shown as chips above the input (removable) | Cl, Co | ⬜ | S | |
| AI-21 | `@` mentions: notes (the `[[` suggestion popup), folders, tags; `@` web pages by URL | Cl, Co | ⬜ | M | Reuses the link suggestions (W-23) |
| AI-22 | Images (attachments in the vault) and PDFs as context | Cl, Co | 🟡 | M | Sent to models that take them; PDFs as text where the model doesn't |
| AI-23 | Instructions: a system prompt in the settings, and the vault's own (`CLAUDE.md` / an instructions note) read by the agent | Cl, Co | ⬜ | S | |
| AI-24 | Projects: a named set of instructions, context notes and its own chats | Co | ⬜ | M | |
| AI-25 | YouTube transcripts as context | Co | ✗ | – | Needs a transcript service; left out for now |

## 4. Working in the note

| ID | Feature | From | Now | Effort | Notes |
|----|---------|------|-----|--------|-------|
| AI-30 | Inline edit: select text (or at the cursor), a key, type what to change; the change is shown as a word-level diff in the note, Enter accepts, Esc rejects; one undo step | Cl | ⬜ | L | The diff needs a generic mdedit API for showing a proposed change (inserted / deleted spans) |
| AI-31 | Quick Ask: a question about the selection in a popup, the answer below it; insert, replace the selection, copy, or continue in the chat | Co | ⬜ | M | |
| AI-32 | Agent edits to notes: shown as a diff before they're written (or after, with undo), per the approval mode | Cl, Co | ⬜ | M | Open tabs of changed notes reload |

## 5. Commands and skills

| ID | Feature | From | Now | Effort | Notes |
|----|---------|------|-----|--------|-------|
| AI-40 | Custom commands: prompts saved as notes in a folder (the file name is the command), each on the palette and as `/name` in the chat | Co, Cl | ⬜ | M | |
| AI-41 | Variables in them: `{}` (the selection or the note), `{activeNote}`, `{[[Note]]}`, `{Folder/Path}`, `{#tag}` | Co | ⬜ | S | |
| AI-42 | Output: insert at the cursor, replace the selection, copy, or open in the chat | Co | ⬜ | S | |
| AI-43 | A few built-in commands to start from: fix grammar, summarize, translate, explain, continue writing | Co | ⬜ | S | Saved as notes the user can change |
| AI-44 | Skills (`$name`): the agent's own skills, from the user's and the vault's folders | Cl, Co | ⬜ | S | The agent CLI loads them; blackglass lists them for `$` |

## 6. Finding notes

| ID | Feature | From | Now | Effort | Notes |
|----|---------|------|-----|--------|-------|
| AI-50 | Vault questions: the model answers from the vault's notes, citing them as `[[links]]` (lexical search first) | Co | 🟡 | M | blackglass's vault search exists (W-08 …): its best matches go in as context |
| AI-51 | Semantic search: embeddings of the notes (by a provider or locally), kept up to date on save, stored in `.blackglass/plugins/ai/`; exclusions by folder / tag | Co | ⬜ | L | Opt-in: embedding sends every note to the provider unless it's local |
| AI-52 | Relevant notes: for the active note, the most similar ones in a sidebar panel; open, insert a link, add to the chat | Co | ⬜ | M | With AI-51 |

## 7. Safety and control

| ID | Feature | From | Now | Effort | Notes |
|----|---------|------|-----|--------|-------|
| AI-60 | Approval: each file change and command asks first (Allow once / always / Deny), with what it will do | Cl, Co | ⬜ | M | The agent CLI's permission prompts, answered in blackglass |
| AI-61 | Modes: plan (read only), safe (asks), unrestricted (the user turns it on, with a warning) | Cl | ⬜ | S | |
| AI-62 | The agent stays in the vault: its working folder is the vault; files outside need approval | Cl | ⬜ | S | The CLI's own rules plus the approvals |
| AI-63 | A privacy line in the settings: what is sent where, and nothing without a request; usage (tokens) shown per chat | Co | ⬜ | S | |
| AI-64 | Copilot's hosted models and cloud tools, its subscription, diagnostics upload | Co | ✗ | – | Tied to Copilot's service; blackglass talks to providers directly |
| AI-65 | Mobile | Cl, Co | ✗ | – | No mobile app |

## 8. Suggested order

1. **Chat with a model** (M): AI-01 … AI-03, AI-10, AI-11, AI-15, AI-20,
   AI-23, AI-63, the settings page.
2. **Commands in the note** (M): AI-31, AI-40 … AI-43.
3. **Context** (M): AI-21, AI-22, AI-14, AI-13.
4. **Agents** (L): AI-04, AI-05, AI-12, AI-32, AI-44, AI-60 … AI-62.
5. **Inline edit** (L, with a mdedit API): AI-30.
6. **Search** (L): AI-50 … AI-52, AI-24.

## 9. Checked against

- Claudian: [repository and README](https://github.com/YishenTu/claudian),
  [a review of 2.1.4's providers](https://parazettel.com/articles/claudian-obsidian-plugin/)
  (2026-09-30).
- Copilot for Obsidian: [repository](https://github.com/logancyang/obsidian-copilot),
  [documentation](https://github.com/logancyang/obsidian-copilot/blob/master/docs/index.md),
  [custom commands](https://github.com/logancyang/obsidian-copilot/blob/master/docs/custom-commands.md)
  (2026-09-30).
