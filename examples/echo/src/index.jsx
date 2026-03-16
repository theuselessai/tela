var initialState = {
  messages: [],
  input: "",
  status: "connecting...",
  mode: "normal",
};

function reduce(state, action) {
  switch (action.type) {
    case "ws_open":
      return Object.assign({}, state, { status: "connected" });
    case "ws_close":
      return Object.assign({}, state, { status: "disconnected" });
    case "ws_error":
      return Object.assign({}, state, { status: "error: " + action.message });
    case "ws_message":
      return Object.assign({}, state, {
        messages: state.messages.concat([{ dir: "in", text: action.data }]),
      });
    case "sent":
      return Object.assign({}, state, {
        messages: state.messages.concat([{ dir: "out", text: action.text }]),
        input: "",
        mode: "normal",
      });
    case "input_char":
      return Object.assign({}, state, {
        input: state.input + action.char,
      });
    case "input_backspace":
      return Object.assign({}, state, {
        input: state.input.slice(0, -1),
      });
    case "input_submit":
      if (state.input.length > 0) {
        sendMessage(state.input);
      }
      return state;
    case "enter_insert":
      return Object.assign({}, state, { mode: "insert" });
    case "enter_normal":
      return Object.assign({}, state, { mode: "normal" });
    default:
      return state;
  }
}

var keybindings = {
  normal: {
    i: "enter_insert",
    q: "quit",
  },
  insert: {},
};

var ws = new WebSocket("wss://ws.postman-echo.com/raw");

ws.onopen = function () {
  globalThis.__tela_dispatch_queue__.push({ type: "ws_open" });
};

ws.onmessage = function (event) {
  globalThis.__tela_dispatch_queue__.push({ type: "ws_message", data: event.data });
};

ws.onclose = function () {
  globalThis.__tela_dispatch_queue__.push({ type: "ws_close" });
};

ws.onerror = function (event) {
  globalThis.__tela_dispatch_queue__.push({ type: "ws_error", message: event.message || "unknown" });
};

function sendMessage(text) {
  ws.send(text);
  globalThis.__tela_dispatch_queue__.push({ type: "sent", text: text });
}

function view(state, dispatch) {
  var isInsert = state.mode === "insert";

  return (
    <box border="rounded" title={"WebSocket Echo [" + state.status + "]"}>
      <layout direction="vertical">
        <list flex={1}>
          {state.messages.map(function (msg, i) {
            return (
              <text key={i}>
                <span fg={msg.dir === "out" ? "cyan" : "green"} bold={true}>
                  {msg.dir === "out" ? ">>> " : "<<< "}
                </span>
                <span>{msg.text}</span>
              </text>
            );
          })}
        </list>
        <input
          height={1}
          value={state.input}
          placeholder={isInsert ? "Type a message..." : ""}
          fg="white"
        />
        <text height={1} align="center" fg="gray">
          {isInsert
            ? "Enter: send | Escape: normal mode"
            : "i: type message | q: quit"}
        </text>
      </layout>
    </box>
  );
}
