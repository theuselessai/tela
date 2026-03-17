# Pipelit TUI Redesign Plan

## Vision

The TUI is a **situation room** — a terminal-native command center for the Pipelit platform. The agent is a **ghost in the shell**: always present, watching what you're looking at, ready when you need it. Not a screen you go to, but a presence in the system.

Pipelit's purpose is to deflect, reduce noise, process, and automate — so the human can focus on what's important. The TUI is the full-power interface; Telegram, email, and Slack are constrained views into the same system.

## Architecture: Three Layers

```
┌─ tabs (dynamic, open/close) ─────────────────┐
│                                               │
│   content (operational views, full width)      │
│                                               │
├─ visor (the ghost) ──────────────────────────┤
│   chat / commands / status                    │
└───────────────────────────────────────────────┘
```

**No sidebar.** Every pixel goes to content. At 40 columns that's critical.

### 1. Tabs (Top)

Dynamic buffer list, like vim's tabline. No static tabs — you open and close views as needed.

- `:chat <agent>` opens a chat context (or focuses if already open)
- `:exec <id>` opens an execution detail tab
- `:e <workflow>` opens a workflow editor tab
- `gt` / `gT` — cycle next/prev tab
- `:q` — close current tab
- `:ls` — list open buffers
- `:b <number>` — jump to buffer

Last tab closes → back to home. Home is always buffer 0.

On narrow screens, only the active tab name + count is shown: `chat:default (1/4)`. `gt`/`gT` still cycles.

### 2. Content (Middle)

Full-width, single-view operational content. What renders here depends on the active tab/buffer:

- Workflow list (home)
- Workflow editor (nodes, edges, graph)
- Execution detail
- Credentials manager
- Memory browser
- Settings

Each tab has its own view stack (push/pop with `Enter`/`Esc` for drilling into details like node config or execution logs).

### 3. Visor (Bottom)

The core interaction surface. The command bar IS the visor — one element, three behaviors:

**Collapsed** (1 line) — Status bar with live counters:
```
● 2 run  📥 3 in  ⚠ 1 wait  [Default Agent]           localhost:8000 100%
```

**Peek** (3-5 lines) — Expands for interrupts/notifications:
```
▣ tela
⚠ Email from John Chen (urgent) — reply drafted
┃ "Hi John, attached Q1 summary..."
[approve] [edit] [dismiss]
● 2 run  📥 3 in  ⚠ 1 wait                           localhost:8000 100%
```

**Open** (split or full) — Full chat with active agent:
```
▣ agent · claude-opus-4-6
Hi there! How can I help?

▎ Type a message...
● 2 run  📥 3 in  ⚠ 1 wait                           localhost:8000 100%
```

Toggle visor height with a keybind. Preset ratios: collapsed / 25% / 50% / 75% / full.

**The visor unifies three functions:**
- **`:` prefix** → vim command mode with tab completion popup (nvim wildmenu style)
- **No prefix** → chat with the active agent / meta agent
- **Status bar** → passive live counters when collapsed

## Navigation: The Vim Way

No command palette. No overlays. The command bar IS the navigation.

### Mode System

Three modes, same as vim:

- **Normal** — navigate, scroll, select
- **Insert** — text input (chat in visor, form fields in content)
- **Command** — `:` prefix, entered from normal mode

### Command Line with Tab Completion

Tab triggers a floating completion popup (nvim wildmenu style) anchored just above the command line:

```
              ┌─────────────────────────┐
              │ ▸ default-agent         │
              │   data-pipeline         │
              │   email-triage          │
              └─────────────────────────┘
:chat def█
```

Fuzzy filters as you type. `Tab` / `j`/`k` to move selection. `Enter` to confirm and advance to the next argument:

```
              ┌─────────────────────────┐
              │ ▸ Mar 17, 10:30  (3)    │
              │   Mar 16, 14:20  (12)   │
              │   Mar 15, 09:00  (28)   │
              │   + New session          │
              └─────────────────────────┘
:chat default-agent █
```

### Command Vocabulary

```
Navigation:
  :chat <agent> <session>   Switch chat context
  :chat <agent> +new        New session
  :e <workflow>             Open workflow editor
  :exec                     Executions list
  :exec <id>                Open specific execution
  :node <node_id>           Open node config
  :creds                    Credentials
  :mem                      Memory
  :set <key> <value>        Settings

Buffer management:
  :ls                       List open buffers
  :ls chats                 List active chat sessions
  :ls agents                List available agents
  :ls exec                  List recent executions
  :ls nodes                 List nodes in current workflow
  :ls creds                 List credentials
  :ls mem facts             List memory facts
  :b <number>               Jump to buffer
  :q                        Close current buffer
  :qa                       Close all

Actions:
  :approve <id>             Approve pending item
  :reject <id>              Reject pending item
  :cancel <id>              Cancel execution
  :pause <trigger>          Pause a gateway trigger
  :focus                    Pause notifications

Search:
  /                         Search within current view
```

### Global Keybindings (Normal Mode)

```
j/k                     Up/down within current view
Enter / l               Push (drill into detail)
Esc / h                 Pop (go back)
gt / gT                 Next/prev tab
Tab                     Switch agent in visor (fuzzy picker popup)
Ctrl+^                  Switch to last context
G / g                   Jump to bottom/top
:                       Enter command mode
i / a                   Enter insert mode (context-dependent)
/                       Search
?                       Help / keybinding cheat sheet
```

## Agent Switching

In the visor, `Tab` opens a fuzzy picker popup showing only agents with `trigger_chat` nodes:

```
              ┌─────────────────────────┐
              │ ▸ Default Agent     (3) │
              │   Email Triage      (1) │
              │   Slack Bot          (5) │
              └─────────────────────────┘
▎ Type a message...        [Default Agent ▸ Tab]
```

Start typing to fuzzy filter. This keeps the list small — most workflows are background jobs without chat triggers.

## The Meta Agent (Home)

Home (buffer 0) is the meta agent — a conversational interface that manages the platform. The visor's chat, when on the home tab, talks to the meta agent ("tela").

The meta agent can:
- Create/delete workflows
- Configure nodes, credentials, schedules
- Show status summaries
- Change settings
- Approve/reject pending items
- Report on inflow and executions

### Live Status

The visor's collapsed status bar shows live counters (updated via WebSocket):

```
● 2 running  ⚡ 3 triggers  📥 7 inflow  ⚠ 1 awaiting
```

- **running** — active executions
- **triggers** — active gateway listeners (Telegram, email, etc.)
- **inflow** — queued items from gateways (e.g., 2 emails, 3 Telegram messages)
- **awaiting** — items needing human approval (interrupts, drafts, reviews)

### Interrupts

The meta agent proactively surfaces high-priority items. When something needs attention, the visor peeks automatically:

```
▣ tela
⚠ Email from John Chen (urgent) — reply drafted
┃ "Hi John, attached Q1 summary..."
[approve] [edit] [dismiss]
```

Priority levels:
- **Low** — silently increments the inflow counter
- **Medium** — one-line notification in visor
- **High / needs approval** — visor peeks with action buttons

`:focus` pauses all interrupts. The meta agent batches them for later.

### Cross-Gateway Continuity

The user is the same person across TUI, Telegram, email, Slack. The meta agent maintains conversation threads across all interfaces. Approve from TUI, John gets an email. Each interface has its own limitations — TUI is full power, others are constrained views.

| Interface | Can do | Can't do |
|-----------|--------|----------|
| TUI | Everything | — |
| Telegram | Chat, approve/reject, quick replies | Edit nodes, build workflows |
| Email | Reply, approve with keywords, receive summaries | Real-time anything |
| Slack | Chat, approve, threads | Deep config, workflow editing |

## Responsive Layout

Three breakpoints. Same logical hierarchy, different spatial arrangement.

### Narrow (<50 columns)

Single view, full screen. Tab bar shows active tab + count. Visor is collapsed or full — no split.

### Medium (50-120 columns)

Tab bar + content + visor. Visor can split.

### Wide (120+ columns)

Same structure, more room for content. Workflow editor can show graph at full width.

## Workflow Graph Editor

### Node Taxonomy

**Triggers** (entry points — always at the top):
`trigger_chat`, `trigger_telegram`, `trigger_manual`, `trigger_schedule`, `trigger_workflow`, `trigger_error`

**AI Agents** (executable, carry sub-components):
- `agent`, `deep_agent` — require: llm + tool + skill
- `categorizer`, `router`, `extractor` — require: llm + output_parser

**Sub-components** (non-executable, shown inside parent box):
`ai_model`, `run_command`, `output_parser`, `memory_read`, `memory_write`, `code_execute`, `platform_api`, `whoami`, `epic_tools`, `task_tools`, `spawn_and_await`, `workflow_create`, `workflow_discover`, `skill`

**Logic/Flow** (executable):
`switch`, `code`, `merge`, `filter`, `loop`, `wait`, `human_confirmation`, `workflow` (subworkflow)

**Memory**: `identify_user`

### Edge Labels

| Edge Label | Meaning | Visual |
|-----------|---------|--------|
| `""` (empty) | Data flow | Main spine connectors |
| `"llm"` | LLM config | Inside parent box tree |
| `"tool"` | Tool attachment | Inside parent box tree |
| `"output_parser"` | Parser config | Inside parent box tree |
| `"skill"` | Skill attachment | Inside parent box tree |
| `"loop_body"` | Loop entry | Forward edge into loop |
| `"loop_return"` | Loop return | Back-arrow on right side |
| condition_value | Switch branch | Fork bar with labels |

### Graph Layout: Top-Down with Box-Drawing

Approach chosen after evaluating:
- **Canvas with braille markers** — smooth lines, good for overview, but rough at detail level
- **Box-drawing with `<layout position="absolute">`** — crisp, readable, handles complex topology ✓

Layout rules:
1. **Spine** runs top to bottom, centered horizontally
2. **Triggers** at the very top (double border)
3. **Agent boxes** are tall — sub-components rendered as a tree inside the box
4. **Switch** forks into parallel branches via `┌──┴──┐` bar with condition labels
5. **Branches** laid out side-by-side at same Y level
6. **Merge** joins branches via `└──┬──┘` bar
7. **Loop back-edge** rendered as right-side `◀──╮ / │ / ──╯` arrow

### Sub-Component Tree (Inside Agent Box)

Sub-components are grouped by edge label with tree connectors:

```
╭ deep_agent ──────────╮
│deep_agent             │
│                       │
│ ● llm                 │
│   ╰─ claude-opus      │
│ ● tool                │
│   ├─ run_command      │
│   ├─ memory_read      │
│   ├─ memory_write     │
│   ├─ platform_api     │
│   ├─ epic_tools       │
│   ╰─ spawn_and_await  │
│ ● skill               │
│   ╰─ skill            │
╰───────────────────────╯
```

### Reusable JSX Components (planned)

```jsx
<AgentNode label="deep_agent" subs={subs} edges={edges} selected={sel} />
<TriggerNode label="trigger_chat" selected={sel} />
<FlowNode label="switch" type="switch" selected={sel} />
```

Each component handles border style, color, tree rendering, selection state. The graph layout only positions them.

### Navigation Within Graph

- `j/k` — move selection between nodes
- `HJKL` — pan the viewport
- `Enter` on an agent box — enter focus mode for internal tree navigation
- `Esc` — pop back to graph navigation

## View Breakdown

### Home (Workflow List)

| Data | Narrow | Medium | Wide |
|------|--------|--------|------|
| Name | Full | Full | Full |
| Status | Dot icon | Badge | Badge |
| Nodes/Edges | Hidden | Count | Count |
| Created | Hidden | Hidden | Date |

### Workflow Detail

Hub with sub-views: Chat, Nodes, Edges, Executions, Settings.
On narrow: menu list, push/pop. On wide: sidebar menu + inline active sub-view.

### Node Detail / Config

Form-style view. Field types: Toggle (`Space`), Text input (`Enter`), Select (`Enter` → list), Long text (push full-screen editor), JSON (same).

### Executions List

Table: ID, workflow, status, started, completed. Filter via `f`.

### Execution Detail

Overview + node execution logs table. Each log row expandable.

### Credentials

Table: name, type, provider, status. Add, test (LLM), delete.

### Memory

Sub-tabs: `1` Facts, `2` Episodes, `3` Checkpoints, `4` Procedures, `5` Users.

### Settings

Form-style: connection, platform config, logging.

## Technical Implementation

### Tela Engine Changes

**`<popup>` element** — exposes ratatui's overlay rendering as a JSX primitive. Already implemented in `renderer/popup.rs`. Clears rect, renders children on top of frame.

**`<layout position="absolute">`** — positions children at x,y coordinates. Already implemented as a `position` prop on `<layout>` (CSS mental model, not a separate element).

### Everything Else is JS/JSX

| Component | Approach |
|-----------|----------|
| Tab manager | State: `[{id, type, title, viewState}]` |
| Visor | `<layout>` with dynamic `height` based on visor state |
| Completion popup | `<popup>` + `<list>` + fuzzy match logic |
| Command parser | Pure JS: split on spaces, context-aware arg completion |
| Fuzzy matcher | Pure JS: simple substring/prefix scoring |
| View router | State: which component renders per tab |
| Scrollable chat | `<list>` + `selected` offset (existing pattern) |
| Focus management | `state.focus` field, key routing in `reduce()` |
| API client | `fetch` + `websocket` (existing native APIs) |
| Responsive | `Tela.columns` / `Tela.rows` (existing) |
| Graph layout | Sugiyama-style top-down, pure JS |
| Node components | Reusable JSX: AgentNode, TriggerNode, FlowNode |

### Build Order

| Phase | What | Depends on |
|-------|------|------------|
| 0 | `<popup>` element in Rust | Done ✓ |
| 0 | `<layout position="absolute">` | Done ✓ |
| 1 | Shell: tabs + visor + content | popup |
| 2 | Command parser + completer | Shell |
| 3 | Visor chat (agent switching, Tab) | Shell |
| 4 | API client (HTTP + WebSocket) | Nothing |
| 5 | Home view (workflow list) | Shell, API |
| 6 | Chat view (messages, input) | Visor, API |
| 7 | Workflow graph editor | Shell, API |
| 8 | Executions (list + detail) | Shell, API |
| 9 | Credentials, Memory, Settings | Shell, API |
| 10 | Meta agent integration | All above |

## Prototypes Built

Working examples in `examples/`:

- **`graph-boxes/`** — Box-drawing graph with full topology: triggers, agents with sub-component trees, switch/fork/merge, loop back-arrows, 3-way branching. All node types represented.
- **`graph-canvas/`** — Braille canvas approach. Smooth lines, thin outlines. Good for overview/zoomed-out view. Top-down layout with orthogonal routing.

Both demonstrate that the entire graph editor can be built in JSX with no new Rust primitives beyond `popup` and `position="absolute"` on layout.
