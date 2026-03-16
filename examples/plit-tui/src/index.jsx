// ============================================================================
// plit-tui — Pipelit Chat Client for Tela
// ============================================================================

var authData = null;
try {
  var raw = Tela.readFile("~/.config/plit/auth.json");
  if (raw) authData = JSON.parse(raw);
} catch(e) {
  console.error("Failed to read auth config:", e);
}

var PIPELIT_URL = (authData && authData.pipelit_url) || env.get("PIPELIT_URL") || "http://localhost:8000";
var PIPELIT_TOKEN = (authData && authData.token) || env.get("PIPELIT_TOKEN") || "";

// --- Constants --------------------------------------------------------------

var SPINNER_FRAMES = ["\u280B", "\u2819", "\u2839", "\u2838", "\u283C", "\u2834", "\u2826", "\u2827", "\u2807", "\u280F"];

// --- Initial State ----------------------------------------------------------

var initialState = {
  mode: "normal",        // "normal" or "insert" (engine-level)
  activeTab: 0,          // 0=workflows, 1=chat
  tabTitles: ["Workflows", "Chat"],

  // Workflow list
  workflows: [],
  selectedAgent: 0,
  agentName: "",
  modelName: "",

  // Chat
  messages: [],          // [{role: "user"|"assistant", content: "..."}]
  input: "",
  cursor: 0,
  messageQueue: [],      // queued while nodes running

  // Scroll
  scrollOffset: 0,
  stickyBottom: true,
  unreadCount: 0,

  // Execution status
  nodesRunning: false,
  activity: [],          // [{nodeName, status}]
  toolCalls: [],         // [{toolName, nodeId, status}]

  // Connection
  wsStatus: "disconnected",
  hostDisplay: PIPELIT_URL,

  // Command bar — truthy string means command mode (e.g. ":" or ":q")
  command: "",

  // Spinner
  spinnerFrame: 0,
};

// --- Keybindings ------------------------------------------------------------
// Command mode piggybacks on "insert" mode (engine dispatches input_char etc.)
// and is distinguished by state.command being truthy.

var keybindings = {
  normal: {
    ":": "enter_command",
    i: "enter_insert",
    a: "enter_insert",
    q: "quit",
    j: "nav_down",
    k: "nav_up",
    G: "scroll_bottom",
    g: "scroll_top",
    Tab: "tab_next",
    Enter: "agent_select",
  },
  insert: {},
};

// --- Helpers ----------------------------------------------------------------

function wrapText(text, width) {
  if (width <= 0 || text.length === 0) return [text];
  var result = [];
  var words = text.split(" ");
  var line = "";
  for (var i = 0; i < words.length; i++) {
    if (line.length === 0) {
      line = words[i];
    } else if (line.length + 1 + words[i].length > width) {
      result.push(line);
      line = words[i];
    } else {
      line = line + " " + words[i];
    }
  }
  if (line.length > 0) result.push(line);
  return result.length > 0 ? result : [""];
}

// --- API Helpers ------------------------------------------------------------

function sendMessage(slug, text) {
  fetch(PIPELIT_URL + "/api/v1/workflows/" + slug + "/chat/", {
    method: "POST",
    headers: {
      "Authorization": "Bearer " + PIPELIT_TOKEN,
      "Content-Type": "application/json",
    },
    body: JSON.stringify({ text: text }),
  }).then(function(res) {
    if (res.ok) {
      var data = res.json();
      if (data.execution_id && globalThis.__plit_ws__) {
        globalThis.__plit_ws__.send(JSON.stringify({
          type: "subscribe",
          channel: data.execution_id,
        }));
      }
    }
  });
}

function fetchHistory(slug) {
  fetch(PIPELIT_URL + "/api/v1/workflows/" + slug + "/chat/history?limit=200", {
    headers: { Authorization: "Bearer " + PIPELIT_TOKEN },
  }).then(function(res) {
    if (res.ok) {
      var data = res.json();
      var msgs = (data.messages || []).map(function(m) {
        return { role: m.role, content: m.content || m.text || "" };
      });
      Tela.dispatch({ type: "chat_history_loaded", messages: msgs });
    }
  });
}

// --- Reducer ----------------------------------------------------------------

function reduce(state, action) {
  switch (action.type) {

    // -- Mode switching --

    case "enter_insert":
      if (state.activeTab !== 1) return state;
      return Object.assign({}, state, { mode: "insert", command: "" });

    case "enter_normal":
      return Object.assign({}, state, { mode: "normal", command: "" });

    case "enter_command":
      return Object.assign({}, state, { mode: "insert", command: ":" });

    // -- Tabs --

    case "tab_next":
      return Object.assign({}, state, {
        activeTab: (state.activeTab + 1) % state.tabTitles.length,
      });

    case "tab_prev":
      return Object.assign({}, state, {
        activeTab: (state.activeTab + state.tabTitles.length - 1) % state.tabTitles.length,
      });

    // -- Navigation --

    case "nav_down":
      if (state.activeTab === 0) {
        var nextIdx = Math.min(state.selectedAgent + 1, Math.max(0, state.workflows.length - 1));
        return Object.assign({}, state, { selectedAgent: nextIdx });
      }
      return Object.assign({}, state, {
        scrollOffset: state.scrollOffset + 1,
        stickyBottom: false,
        unreadCount: 0,
      });

    case "nav_up":
      if (state.activeTab === 0) {
        return Object.assign({}, state, {
          selectedAgent: Math.max(0, state.selectedAgent - 1),
        });
      }
      return Object.assign({}, state, {
        scrollOffset: Math.max(0, state.scrollOffset - 1),
        stickyBottom: false,
      });

    case "scroll_bottom":
      return Object.assign({}, state, { stickyBottom: true, scrollOffset: 0, unreadCount: 0 });

    case "scroll_top":
      return Object.assign({}, state, { scrollOffset: 0, stickyBottom: false });

    // -- Agent selection --

    case "agent_select":
      if (state.activeTab !== 0 || state.workflows.length === 0) return state;
      var wf = state.workflows[state.selectedAgent];
      if (!wf) return state;
      var selSlug = wf.slug || wf.name || "";
      if (selSlug) fetchHistory(selSlug);
      return Object.assign({}, state, {
        activeTab: 1,
        agentName: wf.name || wf.slug || "unnamed",
        modelName: wf.model || "",
        messages: [],
        scrollOffset: 0,
        stickyBottom: true,
        activity: [],
        toolCalls: [],
      });

    // -- Text input --

    case "input_char":
      if (state.command) {
        return Object.assign({}, state, { command: state.command + action.char });
      }
      var icBefore = state.input.slice(0, state.cursor);
      var icAfter = state.input.slice(state.cursor);
      return Object.assign({}, state, {
        input: icBefore + action.char + icAfter,
        cursor: state.cursor + 1,
      });

    case "input_backspace":
      if (state.command) {
        if (state.command.length > 1) {
          return Object.assign({}, state, { command: state.command.slice(0, -1) });
        }
        return Object.assign({}, state, { mode: "normal", command: "" });
      }
      if (state.cursor === 0) return state;
      var bsBefore = state.input.slice(0, state.cursor - 1);
      var bsAfter = state.input.slice(state.cursor);
      return Object.assign({}, state, { input: bsBefore + bsAfter, cursor: state.cursor - 1 });

    case "input_newline":
      if (state.command) return state;
      var nlBefore = state.input.slice(0, state.cursor);
      var nlAfter = state.input.slice(state.cursor);
      return Object.assign({}, state, {
        input: nlBefore + "\n" + nlAfter,
        cursor: state.cursor + 1,
      });

    case "input_submit":
      // Command mode: parse command
      if (state.command) {
        var cmd = state.command.slice(1).trim();
        if (cmd === "q" || cmd === "quit") {
          Tela.quit();
          return state;
        }
        return Object.assign({}, state, { mode: "normal", command: "" });
      }
      // Chat submit
      if (state.input.trim().length === 0) {
        return Object.assign({}, state, { mode: "normal" });
      }
      var submitText = state.input.trim();
      // Queue if nodes are running
      if (state.nodesRunning) {
        return Object.assign({}, state, {
          messageQueue: state.messageQueue.concat([submitText]),
          input: "",
          cursor: 0,
          mode: "normal",
        });
      }
      // Send via API
      var submitSlug = "";
      if (state.workflows.length > 0 && state.workflows[state.selectedAgent]) {
        submitSlug = state.workflows[state.selectedAgent].slug;
      }
      if (submitSlug) sendMessage(submitSlug, submitText);
      return Object.assign({}, state, {
        messages: state.messages.concat([{ role: "user", content: submitText }]),
        input: "",
        cursor: 0,
        mode: "normal",
        stickyBottom: true,
      });

    // -- WebSocket --

    case "ws_connected":
      return Object.assign({}, state, { wsStatus: "connected" });

    case "ws_disconnected":
      return Object.assign({}, state, { wsStatus: "disconnected" });

    case "ws_message": {
      var wsData = action.data;
      if (!wsData || !wsData.type) return state;

      switch (wsData.type) {
        case "chat_message": {
          var content = wsData.content || wsData.text || "";
          var role = wsData.role || "assistant";
          var newMsgs = state.messages.concat([{ role: role, content: content }]);
          var newUnread = state.stickyBottom ? 0 : state.unreadCount + 1;
          return Object.assign({}, state, { messages: newMsgs, unreadCount: newUnread });
        }

        case "node_status": {
          var nodeName = wsData.node_name || wsData.node_id || "";
          var updatedActivity = state.activity.slice();
          var found = false;
          for (var ni = 0; ni < updatedActivity.length; ni++) {
            if (updatedActivity[ni].nodeName === nodeName) {
              updatedActivity[ni] = { nodeName: nodeName, status: wsData.status };
              found = true;
              break;
            }
          }
          if (!found) {
            updatedActivity.push({ nodeName: nodeName, status: wsData.status });
          }
          var updatedModel = state.modelName;
          if (wsData.model) updatedModel = wsData.model;
          return Object.assign({}, state, {
            activity: updatedActivity,
            nodesRunning: true,
            modelName: updatedModel,
          });
        }

        case "tool_call": {
          var newToolCalls = state.toolCalls.concat([{
            toolName: wsData.tool_name || "",
            nodeId: wsData.node_id || "",
            status: wsData.status || "",
          }]);
          return Object.assign({}, state, { toolCalls: newToolCalls });
        }

        case "execution_started":
          return Object.assign({}, state, { nodesRunning: true, activity: [], toolCalls: [] });

        case "execution_completed":
        case "execution_failed":
        case "execution_done": {
          var flushed = state.messageQueue.slice();
          var doneState = Object.assign({}, state, {
            nodesRunning: false,
            activity: [],
            toolCalls: [],
            messageQueue: [],
          });
          // Flush queued messages
          if (flushed.length > 0) {
            var nextQMsg = flushed[0];
            var restQueue = flushed.slice(1);
            doneState.messageQueue = restQueue;
            doneState.messages = doneState.messages.concat([{ role: "user", content: nextQMsg }]);
            doneState.nodesRunning = true;
            var fSlug = "";
            if (state.workflows.length > 0 && state.workflows[state.selectedAgent]) {
              fSlug = state.workflows[state.selectedAgent].slug;
            }
            if (fSlug) sendMessage(fSlug, nextQMsg);
          }
          return doneState;
        }

        default:
          return state;
      }
    }

    // -- Data loading --

    case "workflows_loaded": {
      var wfs = action.workflows || [];
      var firstName = wfs.length > 0 ? (wfs[0].name || wfs[0].slug || "") : "";
      var firstModel = "";
      if (wfs.length > 0 && wfs[0].nodes) {
        for (var n = 0; n < wfs[0].nodes.length; n++) {
          if (wfs[0].nodes[n].model) {
            firstModel = wfs[0].nodes[n].model;
            break;
          }
        }
      }
      return Object.assign({}, state, {
        workflows: wfs,
        selectedAgent: 0,
        agentName: firstName,
        modelName: firstModel,
      });
    }

    case "chat_history_loaded": {
      var histMsgs = (action.messages || []).map(function(m) {
        return { role: m.role, content: m.content || m.text || "" };
      });
      return Object.assign({}, state, {
        messages: histMsgs,
        scrollOffset: 0,
        stickyBottom: true,
      });
    }

    // -- Spinner --

    case "spinner_tick":
      return Object.assign({}, state, {
        spinnerFrame: (state.spinnerFrame + 1) % SPINNER_FRAMES.length,
      });

    default:
      return state;
  }
}

// --- Components -------------------------------------------------------------

function AgentList({ state }) {
  if (state.workflows.length === 0) {
    return (
      <box border="single" title="Workflows" flex={1}>
        <text align="center" fg="gray">
          {PIPELIT_TOKEN ? "Loading workflows..." : "Set PIPELIT_TOKEN to connect"}
        </text>
      </box>
    );
  }
  return (
    <box border="single" title={"Workflows (" + state.workflows.length + ")"} flex={1}>
      <list selected={state.selectedAgent} highlight_fg="cyan" highlight_symbol="">
        {state.workflows.map(function(wf, i) {
          var marker = i === state.selectedAgent ? "\u25C9 " : "\u25CB ";
          var name = wf.name || wf.slug || "unnamed";
          return <text key={i}>{marker + name}</text>;
        })}
      </list>
    </box>
  );
}

function MessageList({ state }) {
  if (state.messages.length === 0) {
    return (
      <box border="single" title="Chat" flex={1}>
        <text align="center" fg="gray">
          {state.agentName ? "No messages yet. Press i to type." : "Select a workflow first."}
        </text>
      </box>
    );
  }

  // Build display lines from messages with word wrapping
  var chatWidth = Tela.columns > 99 ? Tela.columns - 36 : Tela.columns - 4;
  var contentWidth = Math.max(chatWidth - 4, 10);
  var items = [];

  for (var i = 0; i < state.messages.length; i++) {
    var msg = state.messages[i];
    // Role header
    items.push({ type: "header", role: msg.role });
    // Content lines — pre-wrap for reliable scroll
    var rawLines = msg.content.split("\n");
    for (var j = 0; j < rawLines.length; j++) {
      if (rawLines[j].length === 0) {
        items.push({ type: "content", text: "" });
      } else if (rawLines[j].length <= contentWidth) {
        items.push({ type: "content", text: "  " + rawLines[j] });
      } else {
        var wrapped = wrapText(rawLines[j], contentWidth);
        for (var k = 0; k < wrapped.length; k++) {
          items.push({ type: "content", text: "  " + wrapped[k] });
        }
      }
    }
    items.push({ type: "blank", text: "" });
  }

  // Auto-scroll via list selection
  var selectedIdx;
  if (state.stickyBottom) {
    selectedIdx = Math.max(0, items.length - 1);
  } else {
    selectedIdx = Math.min(state.scrollOffset, Math.max(0, items.length - 1));
  }

  return (
    <box border="single" title="Chat" flex={1}>
      <list selected={selectedIdx} highlight_symbol="">
        {items.map(function(item, idx) {
          if (item.type === "header") {
            if (item.role === "user") {
              return (
                <text key={idx}>
                  <span fg="cyan" bold={true}>{"\u25C7 you"}</span>
                </text>
              );
            }
            return (
              <text key={idx}>
                <span fg="green" bold={true}>{"\u25A3 agent"}</span>
              </text>
            );
          }
          return <text key={idx}>{item.text || " "}</text>;
        })}
      </list>
    </box>
  );
}

function InputBox({ state }) {
  var isActive = state.mode === "insert" && !state.command;
  var title = "Input";
  if (isActive) title = "Input [INSERT]";
  if (state.nodesRunning && state.messageQueue.length > 0) {
    title = title + " (" + state.messageQueue.length + " queued)";
  }
  return (
    <box border="single" title={title} height={3}>
      <textarea
        value={state.input}
        cursor={state.cursor}
        placeholder={isActive ? "Type a message... (Ctrl+J newline)" : "Press i to type"}
        fg="white"
      />
    </box>
  );
}

function ChatView({ state }) {
  if (Tela.columns > 99) {
    return (
      <layout direction="horizontal" flex={1}>
        <box border="single" title="Agent" width={30}>
          <layout direction="vertical">
            <text bold={true} fg="cyan">{state.agentName || "No agent"}</text>
            {state.modelName ? <text fg="gray">{state.modelName}</text> : null}
            <text height={1} />
            {state.activity.length > 0 ? (
              <layout direction="vertical">
                <text fg="yellow" bold={true}>Activity:</text>
                {state.activity.map(function(a, ai) {
                  var color = a.status === "completed" ? "green"
                    : a.status === "running" ? "yellow" : "gray";
                  return <text key={ai} fg={color}>{"  " + a.nodeName + ": " + a.status}</text>;
                })}
              </layout>
            ) : null}
            {state.toolCalls.length > 0 ? (
              <layout direction="vertical">
                <text fg="magenta" bold={true}>Tools:</text>
                {state.toolCalls.slice(-5).map(function(tc, ti) {
                  return <text key={ti} fg="gray">{"  " + tc.toolName}</text>;
                })}
              </layout>
            ) : null}
          </layout>
        </box>
        <layout direction="vertical" flex={1}>
          <MessageList state={state} />
          <InputBox state={state} />
        </layout>
      </layout>
    );
  }
  return (
    <layout direction="vertical" flex={1}>
      <MessageList state={state} />
      <InputBox state={state} />
    </layout>
  );
}

function ToolBar({ state }) {
  if (state.nodesRunning) {
    var frame = SPINNER_FRAMES[state.spinnerFrame];
    var statusText = frame + " running";
    if (state.activity.length > 0) {
      var latest = state.activity[state.activity.length - 1];
      statusText = frame + " " + latest.nodeName;
    }
    return <text height={1} align="right" fg="yellow">{statusText + " "}</text>;
  }
  return <text height={1} align="right" fg="green">{"\u25CF ready "}</text>;
}

function StatusBar({ state }) {
  var modeText;
  var modeColor;
  if (state.command) {
    modeText = " COMMAND ";
    modeColor = "magenta";
  } else if (state.mode === "insert") {
    modeText = " INSERT ";
    modeColor = "green";
  } else {
    modeText = " NORMAL ";
    modeColor = "blue";
  }

  var connText = " " + state.wsStatus + " ";
  var connColor = state.wsStatus === "connected" ? "green" : "red";

  var scrollText = "";
  if (!state.stickyBottom && state.activeTab === 1) {
    scrollText = " [scroll: " + state.scrollOffset + "]";
  }
  if (state.unreadCount > 0) {
    scrollText = scrollText + " (" + state.unreadCount + " new)";
  }

  return (
    <text height={1}>
      <span fg="black" bg={modeColor} bold={true}>{modeText}</span>
      <span fg="black" bg={connColor}>{connText}</span>
      <span fg="gray">{" " + state.hostDisplay + scrollText}</span>
    </text>
  );
}

function CommandBar({ state }) {
  return <text height={1} fg="white">{state.command}</text>;
}

// --- Main View --------------------------------------------------------------

function view(state) {
  return (
    <layout direction="vertical">
      <tabs height={1} selected={state.activeTab} highlight_fg="white" divider=" | ">
        {state.tabTitles.map(function(t, i) { return <text key={i}>{t}</text>; })}
      </tabs>
      {state.activeTab === 0 ? (
        <AgentList state={state} />
      ) : (
        <ChatView state={state} />
      )}
      <ToolBar state={state} />
      <StatusBar state={state} />
      {state.command ? <CommandBar state={state} /> : null}
    </layout>
  );
}

// --- Startup ----------------------------------------------------------------

// Fetch workflows on startup
if (PIPELIT_TOKEN) {
  fetch(PIPELIT_URL + "/api/v1/workflows/", {
    headers: { Authorization: "Bearer " + PIPELIT_TOKEN },
  }).then(function(res) {
    if (res.ok) {
      var data = res.json();
      var items = data.items || data.results || data || [];
      Tela.dispatch({ type: "workflows_loaded", workflows: items });
      if (items.length > 0 && (items[0].slug || items[0].name)) {
        fetchHistory(items[0].slug || items[0].name);
      }
    }
  });
}

// WebSocket connection with auto-reconnect
if (PIPELIT_TOKEN) {
  var wsUrl = PIPELIT_URL.replace("http", "ws") + "/ws/?token=" + PIPELIT_TOKEN;
  function connectWs() {
    var ws = new WebSocket(wsUrl);
    ws.onopen = function() {
      Tela.dispatch({ type: "ws_connected" });
    };
    ws.onclose = function() {
      Tela.dispatch({ type: "ws_disconnected" });
      setTimeout(connectWs, 3000);
    };
    ws.onmessage = function(e) {
      try {
        var msg = JSON.parse(e.data);
        if (msg.type === "ping") {
          ws.send(JSON.stringify({ type: "pong" }));
          return;
        }
        Tela.dispatch({ type: "ws_message", data: msg });
      } catch (err) {
        console.error("WS parse error:", err);
      }
    };
    ws.onerror = function() {
      console.error("WS error");
    };
    globalThis.__plit_ws__ = ws;
  }
  connectWs();
}

// Spinner timer
setInterval(function() { Tela.dispatch({ type: "spinner_tick" }); }, 80);
