var initialState = {
  time: "--:--:--",
  ip: "loading...",
  fetchStatus: "pending",
  ticks: 0,
};

function reduce(state, action) {
  switch (action.type) {
    case "tick":
      var now = new Date();
      var h = String(now.getHours()).padStart(2, "0");
      var m = String(now.getMinutes()).padStart(2, "0");
      var s = String(now.getSeconds()).padStart(2, "0");
      return Object.assign({}, state, {
        time: h + ":" + m + ":" + s,
        ticks: state.ticks + 1,
      });
    case "ip_loaded":
      return Object.assign({}, state, {
        ip: action.ip,
        fetchStatus: "done",
      });
    case "ip_failed":
      return Object.assign({}, state, {
        ip: "failed: " + action.error,
        fetchStatus: "error",
      });
    default:
      return state;
  }
}

var keybindings = {
  normal: {
    r: "refresh_ip",
    q: "quit",
  },
};

var actions = {
  refresh_ip: async function (dispatch) {
    try {
      var res = await fetch("https://httpbin.org/ip");
      var data = res.json();
      dispatch({ type: "ip_loaded", ip: data.origin || "unknown" });
    } catch (e) {
      dispatch({ type: "ip_failed", error: String(e) });
    }
  },
};

setInterval(function () {
  globalThis.__tela_dispatch_queue__.push({ type: "tick" });
}, 1000);

setTimeout(function () {
  actions.refresh_ip(function (action) {
    globalThis.__tela_dispatch_queue__.push(action);
  });
}, 100);

function view(state, dispatch) {
  return (
    <box border="rounded" title="Clock Dashboard">
      <layout direction="vertical">
        <text height={3} align="center" bold={true}>
          {state.time}
        </text>
        <text height={1} align="center" fg="cyan">
          {"Ticks: " + state.ticks}
        </text>
        <text height={1} align="center">
          {" "}
        </text>
        <text height={1} align="center" fg="yellow">
          {"Public IP: " + state.ip}
        </text>
        <gauge
          height={1}
          percent={state.fetchStatus === "done" ? 100 : state.fetchStatus === "error" ? 0 : 50}
          label={state.fetchStatus}
          fg={state.fetchStatus === "done" ? "green" : state.fetchStatus === "error" ? "red" : "yellow"}
        />
        <text height={1} align="center">
          {" "}
        </text>
        <text height={1} align="center" fg="gray">
          r: refresh IP | q: quit
        </text>
      </layout>
    </box>
  );
}
