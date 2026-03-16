var authData = null;
var authPath = "~/.config/plit/auth.json";
try {
  var raw = Tela.readFile(authPath);
  if (raw) authData = JSON.parse(raw);
} catch(e) {}

var PIPELIT_URL = (authData && authData.pipelit_url) || env.get("PIPELIT_URL") || "http://localhost:8000";
var PIPELIT_TOKEN = (authData && authData.token) || env.get("PIPELIT_TOKEN") || "";

var SPINNERS = ["\u280B","\u2819","\u2839","\u2838","\u283C","\u2834","\u2826","\u2827","\u2807","\u280F"];

var initialState = {
  mode: "normal",
  activeTab: 0,
  workflows: [],
  selectedAgent: 0,
  agentName: "",
  modelName: "",
  messages: [],
  input: "",
  cursor: 0,
  messageQueue: [],
  scrollOffset: 0,
  stickyBottom: true,
  unreadCount: 0,
  nodesRunning: false,
  activity: [],
  toolCalls: [],
  wsStatus: "disconnected",
  hostDisplay: PIPELIT_URL,
  command: "",
  spinnerFrame: 0,
};

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

function reduce(state, action) {
  switch (action.type) {
    case "enter_insert":
      return Object.assign({}, state, { mode: "insert", command: "" });

    case "enter_normal":
      return Object.assign({}, state, { mode: "normal", command: "" });

    case "enter_command":
      return Object.assign({}, state, { mode: "insert", command: ":" });

    case "tab_next":
      return Object.assign({}, state, { activeTab: (state.activeTab + 1) % 2 });

    case "nav_down":
      if (state.activeTab === 0) {
        return Object.assign({}, state, {
          selectedAgent: Math.min(state.selectedAgent + 1, Math.max(0, state.workflows.length - 1)),
        });
      }
      return Object.assign({}, state, {
        scrollOffset: state.scrollOffset + 1,
        stickyBottom: false,
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
      return Object.assign({}, state, { stickyBottom: true, unreadCount: 0 });

    case "scroll_top":
      return Object.assign({}, state, { scrollOffset: 0, stickyBottom: false });

    case "agent_select":
      if (state.workflows.length === 0) return state;
      var sel = state.workflows[state.selectedAgent];
      if (!sel) return state;
      fetchHistory(sel.slug);
      return Object.assign({}, state, {
        agentName: sel.name,
        activeTab: 1,
      });

    case "input_char":
      if (state.command) {
        return Object.assign({}, state, { command: state.command + action.char });
      }
      var before = state.input.slice(0, state.cursor);
      var after = state.input.slice(state.cursor);
      return Object.assign({}, state, {
        input: before + action.char + after,
        cursor: state.cursor + 1,
      });

    case "input_backspace":
      if (state.command) {
        if (state.command.length > 1) {
          return Object.assign({}, state, { command: state.command.slice(0, -1) });
        }
        return Object.assign({}, state, { mode: "normal", command: "" });
      }
      if (state.cursor > 0) {
        var bBefore = state.input.slice(0, state.cursor - 1);
        var bAfter = state.input.slice(state.cursor);
        return Object.assign({}, state, {
          input: bBefore + bAfter,
          cursor: state.cursor - 1,
        });
      }
      return state;

    case "input_newline":
      if (state.command) return state;
      var nlBefore = state.input.slice(0, state.cursor);
      var nlAfter = state.input.slice(state.cursor);
      return Object.assign({}, state, {
        input: nlBefore + "\n" + nlAfter,
        cursor: state.cursor + 1,
      });

    case "input_submit":
      if (state.command) {
        var cmd = state.command.slice(1);
        if (cmd === "q" || cmd === "quit") {
          Tela.quit();
          return state;
        }
        return Object.assign({}, state, { mode: "normal", command: "" });
      }
      if (state.input.trim().length === 0) return state;
      if (state.nodesRunning) {
        return Object.assign({}, state, {
          messageQueue: state.messageQueue.concat([state.input]),
          input: "",
          cursor: 0,
        });
      }
      var slug = "";
      if (state.workflows.length > 0 && state.workflows[state.selectedAgent]) {
        slug = state.workflows[state.selectedAgent].slug;
      }
      if (slug) {
        sendMessage(slug, state.input);
      }
      return Object.assign({}, state, {
        messages: state.messages.concat([{ role: "user", content: state.input }]),
        input: "",
        cursor: 0,
      });

    case "ws_connected":
      return Object.assign({}, state, { wsStatus: "connected" });

    case "ws_disconnected":
      return Object.assign({}, state, { wsStatus: "disconnected" });

    case "ws_message":
      var msg = action.data;
      if (!msg || !msg.type) return state;

      if (msg.type === "chat_message") {
        var content = msg.content || msg.text || "";
        var newMsgs = state.messages.concat([{ role: "assistant", content: content }]);
        var newUnread = state.stickyBottom ? state.unreadCount : state.unreadCount + 1;
        return Object.assign({}, state, { messages: newMsgs, unreadCount: newUnread });
      }

      if (msg.type === "node_status") {
        var newActivity = state.activity.slice();
        var found = false;
        for (var i = 0; i < newActivity.length; i++) {
          if (newActivity[i].node_id === msg.node_id) {
            newActivity[i] = msg;
            found = true;
            break;
          }
        }
        if (!found) newActivity.push(msg);
        var newModel = state.modelName;
        if (msg.model) newModel = msg.model;
        return Object.assign({}, state, {
          activity: newActivity,
          nodesRunning: true,
          modelName: newModel,
        });
      }

      if (msg.type === "execution_started") {
        return Object.assign({}, state, {
          nodesRunning: true,
          activity: [],
          toolCalls: [],
        });
      }

      if (msg.type === "execution_completed" || msg.type === "execution_failed") {
        var flushed = state.messageQueue.slice();
        var newState = Object.assign({}, state, {
          nodesRunning: false,
          activity: [],
          toolCalls: [],
          messageQueue: [],
        });
        if (flushed.length > 0) {
          var nextMsg = flushed[0];
          var rest = flushed.slice(1);
          newState.messageQueue = rest;
          newState.messages = newState.messages.concat([{ role: "user", content: nextMsg }]);
          var fSlug = "";
          if (state.workflows.length > 0 && state.workflows[state.selectedAgent]) {
            fSlug = state.workflows[state.selectedAgent].slug;
          }
          if (fSlug) {
            sendMessage(fSlug, nextMsg);
          }
        }
        return newState;
      }

      return state;

    case "workflows_loaded":
      var wfs = action.workflows || [];
      var firstName = wfs.length > 0 ? wfs[0].name : "";
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

    case "chat_history_loaded":
      var histMsgs = (action.messages || []).map(function(m) {
        return { role: m.role, content: m.content || m.text || "" };
      });
      return Object.assign({}, state, {
        messages: histMsgs,
        stickyBottom: true,
      });

    case "spinner_tick":
      return Object.assign({}, state, {
        spinnerFrame: (state.spinnerFrame + 1) % 10,
      });

    default:
      return state;
  }
}

function sendMessage(slug, text) {
  fetch(PIPELIT_URL + "/api/v1/workflows/" + slug + "/chat/", {
    method: "POST",
    headers: { "Authorization": "Bearer " + PIPELIT_TOKEN, "Content-Type": "application/json" },
    body: JSON.stringify({ text: text }),
  }).then(function(res) {
    if (res.ok) {
      var data = res.json();
      if (data.execution_id && globalThis.__plit_ws__) {
        globalThis.__plit_ws__.send(JSON.stringify({ type: "subscribe", channel: data.execution_id }));
      }
    }
  });
}

function fetchHistory(slug) {
  fetch(PIPELIT_URL + "/api/v1/workflows/" + slug + "/chat/history?limit=200", {
    headers: { "Authorization": "Bearer " + PIPELIT_TOKEN },
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

if (PIPELIT_TOKEN) {
  fetch(PIPELIT_URL + "/api/v1/workflows/", {
    headers: { "Authorization": "Bearer " + PIPELIT_TOKEN },
  }).then(function(res) {
    if (res.ok) {
      var data = res.json();
      var items = (data.items || []).filter(function(w) {
        return w.nodes && w.nodes.some(function(n) { return n.component_type === "trigger_chat"; });
      });
      Tela.dispatch({ type: "workflows_loaded", workflows: items });
      if (items.length > 0) {
        fetchHistory(items[0].slug);
      }
    }
  });
}

if (PIPELIT_TOKEN) {
  var wsUrl = PIPELIT_URL.replace("http", "ws") + "/ws/?token=" + PIPELIT_TOKEN;
  function connectWs() {
    var ws = new WebSocket(wsUrl);
    ws.onopen = function() { Tela.dispatch({ type: "ws_connected" }); };
    ws.onclose = function() {
      Tela.dispatch({ type: "ws_disconnected" });
      setTimeout(connectWs, 3000);
    };
    ws.onmessage = function(e) {
      try {
        var msg = JSON.parse(e.data);
        if (msg.type === "ping") { ws.send(JSON.stringify({ type: "pong" })); return; }
        Tela.dispatch({ type: "ws_message", data: msg });
      } catch(err) {}
    };
    globalThis.__plit_ws__ = ws;
  }
  connectWs();
}

setInterval(function() { Tela.dispatch({ type: "spinner_tick" }); }, 80);

function AgentList(props) {
  var state = props.state;
  return (
    <list selected={state.selectedAgent} highlight_fg="white" highlight_symbol="  \u25C9 ">
      {state.workflows.map(function(w, i) {
        return <text key={i}>{"  " + w.name}</text>;
      })}
    </list>
  );
}

function MessageList(props) {
  var state = props.state;
  var lines = [];
  state.messages.forEach(function(msg) {
    if (msg.role === "user") {
      lines.push("  \u25C7 you");
      lines.push("  " + msg.content);
    } else {
      lines.push("  \u25A3 " + state.agentName + " \u00B7 " + state.modelName);
      lines.push("  " + msg.content);
    }
    lines.push("");
  });
  return (
    <text wrap={true} scroll={state.stickyBottom ? 99999 : state.scrollOffset}>
      {lines.join("\n")}
    </text>
  );
}

function InputBox(props) {
  var state = props.state;
  return (
    <textarea
      value={state.input}
      cursor={state.cursor}
      placeholder={"  \u258E Type a message..."}
      fg="white"
      maxHeight={5}
    />
  );
}

function ChatView(props) {
  var state = props.state;
  if (Tela.columns > 99) {
    return (
      <layout direction="horizontal" flex={1}>
        <box border="single" title="Agents" width={30}>
          <AgentList state={state} />
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

function StatusBar(props) {
  var state = props.state;
  var displayMode = state.command ? "command" : state.mode;
  var modeLabel = displayMode === "insert" ? "\u25C6 write" : displayMode === "command" ? "\u25B7 command" : "\u25C7 navigate";
  var connColor = state.wsStatus === "connected" ? "green" : state.wsStatus === "reconnecting" ? "yellow" : "gray";
  var connLabel = state.wsStatus === "connected" ? "\u25CF " + state.hostDisplay : "\u25CB disconnected";
  return (
    <layout direction="horizontal" height={1}>
      <text flex={1} fg="gray">{" " + modeLabel + "  "}<span fg={connColor}>{connLabel}</span></text>
      <text width={12} align="right" fg="gray">{state.unreadCount > 0 ? "\u2193 " + state.unreadCount + " new" : ""}</text>
    </layout>
  );
}

function CommandBar(props) {
  var state = props.state;
  return <text height={1} fg="white">{state.command}</text>;
}

function ToolBar(props) {
  var state = props.state;
  var label = state.nodesRunning
    ? SPINNERS[state.spinnerFrame] + " " + state.agentName
    : "\u25CF ready";
  var color = state.nodesRunning ? "yellow" : "green";
  return <text height={1} align="right" fg={color}>{label + " "}</text>;
}

function view(state) {
  var tabLabels = ["Agents", "Chat"];
  return (
    <layout direction="vertical">
      <box border="none" height={1}>
        <tabs selected={state.activeTab} highlight_fg="yellow">
          {tabLabels.map(function(t, i) {
            return <text key={i} fg="cyan">{" " + t + " "}</text>;
          })}
        </tabs>
      </box>
      {state.activeTab === 0 ? (
        <box border="single" title={state.agentName || "Agents"} flex={1}>
          <AgentList state={state} />
        </box>
      ) : (
        <ChatView state={state} />
      )}
      <ToolBar state={state} />
      <StatusBar state={state} />
      {state.command ? <CommandBar state={state} /> : null}
    </layout>
  );
}
