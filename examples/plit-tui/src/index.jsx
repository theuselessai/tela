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
        return res.json();
      }
     }).then(function(data) {
       if (data && data.execution_id && globalThis.__plit_ws__) {
         globalThis.__plit_ws__.send(JSON.stringify({
           type: "subscribe",
           channel: data.execution_id,
         }));
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
      var maxOffset = Math.max(0, state.messages.length * 5);
      var ndOffset = Math.max(0, Math.min(state.scrollOffset, maxOffset) - 3);
      return Object.assign({}, state, {
        scrollOffset: ndOffset,
        stickyBottom: ndOffset === 0,
        unreadCount: 0,
      });

    case "nav_up":
      if (state.activeTab === 0) {
        return Object.assign({}, state, {
          selectedAgent: Math.max(0, state.selectedAgent - 1),
        });
      }
      return Object.assign({}, state, {
        scrollOffset: state.scrollOffset + 3,
        stickyBottom: false,
      });

    case "scroll_bottom":
      return Object.assign({}, state, { stickyBottom: true, scrollOffset: 0, unreadCount: 0 });

    case "scroll_top":
      return Object.assign({}, state, { scrollOffset: 99999, stickyBottom: false });

    // -- Agent selection --

      case "agent_select":
        if (state.activeTab !== 0 || state.workflows.length === 0) return state;
        var wf = state.workflows[state.selectedAgent];
        if (!wf) return state;
        var selSlug = wf.slug || wf.name || "";
        if (selSlug) fetchHistory(selSlug);
        if (globalThis.__plit_ws__ && globalThis.__plit_ws__.readyState === 1) {
          globalThis.__plit_ws__.send(JSON.stringify({ type: "subscribe", channel: "workflow:" + selSlug }));
        }
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
          var msgData = wsData.data || {};
          var content = msgData.text || msgData.content || "";
          var role = msgData.role || "assistant";
          var newMsgs = state.messages.concat([{ role: role, content: content }]);
          var newUnread = state.stickyBottom ? 0 : state.unreadCount + 1;
          return Object.assign({}, state, { messages: newMsgs, unreadCount: newUnread });
        }

        case "node_status": {
          var nsData = wsData.data || {};
          var nodeName = nsData.node_id || nsData.node_name || "";
          
          // Check if this is a tool call
          if (nsData.is_tool_call) {
            var newToolCalls = state.toolCalls.concat([{
              toolName: nsData.tool_name || "",
              nodeId: nsData.node_id || "",
              status: nsData.status || "",
            }]);
            return Object.assign({}, state, { toolCalls: newToolCalls, nodesRunning: true });
          }
          
          var updatedActivity = state.activity.slice();
          var found = false;
          for (var ni = 0; ni < updatedActivity.length; ni++) {
            if (updatedActivity[ni].nodeName === nodeName) {
              updatedActivity[ni] = { nodeName: nodeName, status: nsData.status };
              found = true;
              break;
            }
          }
          if (!found) {
            updatedActivity.push({ nodeName: nodeName, status: nsData.status });
          }
          var updatedModel = state.modelName;
          if (nsData.model_name) updatedModel = nsData.model_name;
          return Object.assign({}, state, {
            activity: updatedActivity,
            nodesRunning: true,
            modelName: updatedModel,
          });
        }

        case "tool_call": {
          var tcData = wsData.data || {};
          var newToolCalls = state.toolCalls.concat([{
            toolName: tcData.tool_name || "",
            nodeId: tcData.node_id || "",
            status: tcData.status || "",
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
  var items = [];
  if (state.workflows.length === 0) {
    items.push(<text key="empty" fg="gray">  {PIPELIT_TOKEN ? "Loading workflows..." : "Set PIPELIT_TOKEN to connect"}</text>);
  } else {
    for (var i = 0; i < state.workflows.length; i++) {
      var wf = state.workflows[i];
      var name = wf.name || wf.slug || "unnamed";
      var marker = i === state.selectedAgent ? "\u25C9 " : "    ";
      var fg = i === state.selectedAgent ? "white" : "gray";
      items.push(<text key={i} fg={fg}>{marker + name}</text>);
    }
  }
  return (
    <layout direction="vertical" flex={1}>
      {items}
    </layout>
  );
}

function MessageList({ state }) {
  if (state.messages.length === 0) {
    return (
      <layout direction="vertical" flex={1}>
        <text fg="gray">  {state.agentName ? "No messages yet. Press i to type." : "Select a workflow first."}</text>
      </layout>
    );
  }

  var chatWidth = Tela.columns > 80 ? Tela.columns - 32 : Tela.columns - 4;
  var contentWidth = Math.max(chatWidth - 4, 10);
  var items = [];

  for (var i = 0; i < state.messages.length; i++) {
    var msg = state.messages[i];
    
    if (msg.role === "user") {
      items.push({ type: "header", role: "user", text: "\u2503 you" });
      var rawLines = msg.content.split("\n");
      for (var j = 0; j < rawLines.length; j++) {
        if (rawLines[j].length === 0) {
          items.push({ type: "content", role: "user", text: "  \u2503 " });
        } else if (rawLines[j].length <= contentWidth) {
          items.push({ type: "content", role: "user", text: "  \u2503 " + rawLines[j] });
        } else {
          var wrapped = wrapText(rawLines[j], contentWidth);
          for (var k = 0; k < wrapped.length; k++) {
            items.push({ type: "content", role: "user", text: "  \u2503 " + wrapped[k] });
          }
        }
      }
    } else {
      var model = state.modelName || "agent";
      items.push({ type: "header", role: "assistant", text: "\u25A3 agent \u00B7 " + model });
      var rawLines = msg.content.split("\n");
      for (var j = 0; j < rawLines.length; j++) {
        if (rawLines[j].length === 0) {
          items.push({ type: "content", role: "assistant", text: "" });
        } else if (rawLines[j].length <= contentWidth) {
          items.push({ type: "content", role: "assistant", text: "  " + rawLines[j] });
        } else {
          var wrapped = wrapText(rawLines[j], contentWidth);
          for (var k = 0; k < wrapped.length; k++) {
            items.push({ type: "content", role: "assistant", text: "  " + wrapped[k] });
          }
        }
      }
    }
    items.push({ type: "blank", text: "" });
  }

  var effectiveOffset = Math.min(state.scrollOffset, Math.max(0, items.length - 1));
  var selectedIdx;
  if (state.stickyBottom) {
    selectedIdx = Math.max(0, items.length - 1);
  } else {
    selectedIdx = Math.max(0, items.length - 1 - effectiveOffset);
  }

  return (
    <layout direction="vertical" flex={1}>
      <list selected={selectedIdx} highlight_symbol="">
        {items.map(function(item, idx) {
          if (item.type === "header") {
            if (item.role === "user") {
              return (
                <text key={idx}>
                  <span fg="cyan">{"  " + item.text}</span>
                </text>
              );
            }
            return (
              <text key={idx}>
                <span fg="green">{"  " + item.text}</span>
              </text>
            );
          }
          if (item.type === "blank") {
            return <text key={idx}> </text>;
          }
          if (item.type === "content" && item.role === "user") {
            return (
              <text key={idx}>
                <span fg="cyan">{"  \u2503 "}</span>
                <span>{item.text.slice(5)}</span>
              </text>
            );
          }
          return <text key={idx}>{item.text}</text>;
        })}
      </list>
    </layout>
  );
}

function InputBox({ state }) {
  var prefix = "  \u258E ";
  if (state.input.length === 0 && state.mode !== "insert") {
    return <text height={1} fg="gray">{prefix + "Type a message..."}</text>;
  }
  return <input height={1} value={prefix + state.input} cursor={prefix.length + state.cursor} fg="white" placeholder={prefix + "Type a message..."} />;
}

function ChatView({ state }) {
  if (Tela.columns > 80) {
    return (
      <layout direction="horizontal" flex={1}>
        <layout direction="vertical" width={28}>
          <text>{"  \u25C9 " + (state.agentName || "No agent")}</text>
          {state.modelName ? <text fg="gray">{"    " + state.modelName}</text> : null}
          <text height={1} />
          {state.activity.length > 0 ? (
            <layout direction="vertical">
              {state.activity.map(function(a, ai) {
                var color = a.status === "completed" ? "green"
                  : a.status === "running" ? "yellow" : "gray";
                return <text key={ai} fg={color}>{"    " + a.nodeName + ": " + a.status}</text>;
              })}
            </layout>
          ) : null}
          {state.toolCalls.length > 0 ? (
            <layout direction="vertical">
              {state.toolCalls.slice(-5).map(function(tc, ti) {
                return <text key={ti} fg="gray">{"    " + tc.toolName}</text>;
              })}
            </layout>
          ) : null}
        </layout>
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
  var modeIcon, modeText;
  if (state.command) {
    modeIcon = "\u25B7";
    modeText = "command";
  } else if (state.mode === "insert") {
    modeIcon = "\u25C6";
    modeText = "write";
  } else {
    modeIcon = "\u25C7";
    modeText = "navigate";
  }

  var connIcon = state.wsStatus === "connected" ? "\u25CF" : "\u25CB";
  var connFg = state.wsStatus === "connected" ? "green" : "gray";
  
  var hostDisplay = state.hostDisplay || "";
  hostDisplay = hostDisplay.replace(/^https?:\/\//, "");

  var scrollPct = "100%";
  if (!state.stickyBottom && state.activeTab === 1) {
    if (state.scrollOffset >= 99999) {
      scrollPct = "top";
    } else {
      var maxScroll = Math.max(1, state.messages.length * 3);
      var pct = Math.round(100 * (1 - state.scrollOffset / maxScroll));
      scrollPct = Math.max(0, Math.min(99, pct)) + "%";
    }
  }

  return (
    <layout direction="horizontal" height={1}>
      <text>
        <span>{"  " + modeIcon + " " + modeText}</span>
        <span fg={connFg}>{"  " + connIcon + " " + hostDisplay}</span>
      </text>
      <text align="right" flex={1}>{" " + scrollPct + " "}</text>
    </layout>
  );
}

function CommandBar({ state }) {
  if (state.mode === "insert" && state.command) {
    return <text height={1} fg="white">{state.command}</text>;
  }
  return <text height={1}> </text>;
}

// --- Main View --------------------------------------------------------------

function view(state) {
  return (
    <layout direction="vertical">
      <text height={1}>
        <span fg={state.activeTab === 0 ? "white" : "gray"} bold={state.activeTab === 0}>{"  Workflows"}</span>
        <span>{"    "}</span>
        <span fg={state.activeTab === 1 ? "white" : "gray"} bold={state.activeTab === 1}>{"Chat"}</span>
      </text>
      {state.activeTab === 0 ? (
        <AgentList state={state} />
      ) : (
        <ChatView state={state} />
      )}
      <ToolBar state={state} />
      <StatusBar state={state} />
      <CommandBar state={state} />
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
         // Subscribe to current workflow channel
         var state = globalThis.__tela_state__;
         if (state && state.workflows && state.workflows.length > 0) {
           var slug = state.workflows[state.selectedAgent || 0].slug || state.workflows[state.selectedAgent || 0].name || "";
           if (slug) {
             ws.send(JSON.stringify({ type: "subscribe", channel: "workflow:" + slug }));
           }
         }
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
setInterval(function() {
  if (globalThis.__tela_state__ && globalThis.__tela_state__.nodesRunning) {
    Tela.dispatch({ type: "spinner_tick" });
  }
}, 80);
