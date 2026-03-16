var TASKS = [
  "Buy groceries",
  "Clean the house",
  "Write documentation",
  "Fix bug #42",
  "Deploy to production",
  "Review pull requests",
  "Update dependencies",
  "Refactor auth module",
];

var LOGS = [
  ["INFO", "Server started on port 3000"],
  ["WARN", "Cache miss for key 'user:42'"],
  ["ERROR", "Connection refused to db:5432"],
  ["INFO", "Request handled in 12ms"],
  ["DEBUG", "Token refreshed for session abc"],
  ["INFO", "Backup completed successfully"],
  ["WARN", "Rate limit approaching for /api/v2"],
  ["ERROR", "Timeout waiting for response"],
  ["INFO", "New user registered: alice@example.com"],
  ["DEBUG", "GC cycle completed in 3ms"],
];

var SERVERS = [
  { name: "NorthAmerica", status: "Up", location: "New York" },
  { name: "Europe", status: "Degraded", location: "London" },
  { name: "Asia", status: "Up", location: "Singapore" },
  { name: "SouthAmerica", status: "Down", location: "São Paulo" },
  { name: "Africa", status: "Up", location: "Cape Town" },
];

var BAR_LABELS = ["B1", "B2", "B3", "B4", "B5", "B6", "B7", "B8", "B9", "B10", "B11", "B12"];

function randomSparkline(len) {
  var arr = [];
  for (var i = 0; i < len; i++) arr.push(Math.floor(Math.random() * 100));
  return arr;
}

function randomBarData() {
  return BAR_LABELS.map(function(label) {
    return [label, Math.floor(Math.random() * 50)];
  });
}

function sineWave(offset, len, freq, amp) {
  var pts = [];
  for (var i = 0; i < len; i++) {
    var x = (i + offset) * 0.1;
    pts.push([x, amp * Math.sin(x * freq) + amp]);
  }
  return pts;
}

var initialState = {
  mode: "normal",
  activeTab: 0,
  taskIndex: 0,
  progress: 0.0,
  tick: 0,
  sparkData: randomSparkline(60),
  barData: randomBarData(),
  sinOffset: 0,
};

function reduce(state, action) {
  switch (action.type) {
    case "tick":
      var newProgress = state.progress + 0.01;
      if (newProgress > 1.0) newProgress = 0.0;
      return Object.assign({}, state, {
        tick: state.tick + 1,
        progress: newProgress,
        sparkData: state.sparkData.slice(1).concat([Math.floor(Math.random() * 100)]),
        sinOffset: state.sinOffset + 1,
        barData: state.tick % 10 === 0 ? randomBarData() : state.barData,
      });
    case "next_tab":
      return Object.assign({}, state, { activeTab: (state.activeTab + 1) % 3 });
    case "prev_tab":
      return Object.assign({}, state, { activeTab: (state.activeTab + 2) % 3 });
    case "move_down":
      return Object.assign({}, state, {
        taskIndex: Math.min(state.taskIndex + 1, TASKS.length - 1),
      });
    case "move_up":
      return Object.assign({}, state, {
        taskIndex: Math.max(state.taskIndex - 1, 0),
      });
    default:
      return state;
  }
}

var keybindings = {
  normal: {
    Tab: "next_tab",
    j: "move_down",
    k: "move_up",
    q: "quit",
  },
};

setInterval(function() {
  Tela.dispatch({ type: "tick" });
}, 200);

function Tab0({ state }) {
  var percent = Math.round(state.progress * 100);
  return (
    <layout direction="vertical">
      <layout direction="horizontal" height={7}>
        <box border="rounded" title="Gauge" flex={1}>
          <gauge percent={percent} label={percent + "%"} fg="cyan" />
        </box>
        <box border="rounded" title="Sparkline" flex={1}>
          <sparkline data={state.sparkData} fg="green" />
        </box>
      </layout>
      <box border="rounded" title="Progress" height={3}>
        <linegauge ratio={state.progress} label={percent + "%"} fg="magenta" />
      </box>
      <layout direction="horizontal" flex={1}>
        <box border="rounded" title="Tasks" flex={1}>
          <list selected={state.taskIndex} highlight_fg="yellow" highlight_symbol="> ">
            {TASKS.map(function(t, i) { return <text key={i}>{t}</text>; })}
          </list>
        </box>
        <box border="rounded" title="Logs" flex={1}>
          <list>
            {LOGS.map(function(log, i) {
              var level = log[0];
              var msg = log[1];
              var color = level === "ERROR" ? "red" : level === "WARN" ? "yellow" : level === "DEBUG" ? "gray" : "green";
              return (
                <text key={i}>
                  <span fg={color} bold={true}>{level.padEnd(6)}</span>
                  <span>{msg}</span>
                </text>
              );
            })}
          </list>
        </box>
      </layout>
    </layout>
  );
}

function Tab1({ state }) {
  return (
    <layout direction="horizontal">
      <box border="rounded" title="Servers" flex={1}>
        <table header={["Name", "Status", "Location"]} widths={[20, 12, 15]}>
          {SERVERS.map(function(s, i) {
            var color = s.status === "Up" ? "green" : s.status === "Degraded" ? "yellow" : "red";
            return (
              <row key={i}>
                <text>{s.name}</text>
                <text fg={color} bold={true}>{s.status}</text>
                <text>{s.location}</text>
              </row>
            );
          })}
        </table>
      </box>
      <box border="rounded" title="Network" flex={1}>
        <layout direction="vertical">
          <box border="single" title="Throughput" flex={1}>
            <chart
              xBounds={[state.sinOffset * 0.1, (state.sinOffset + 60) * 0.1]}
              yBounds={[0, 100]}
              xLabels={[String(Math.round(state.sinOffset * 0.1)), "", String(Math.round((state.sinOffset + 60) * 0.1))]}
              yLabels={["0", "50", "100"]}
            >
              <dataset
                name="rx"
                data={sineWave(state.sinOffset, 60, 1.0, 40)}
                fg="cyan"
                marker="braille"
              />
              <dataset
                name="tx"
                data={sineWave(state.sinOffset, 60, 0.7, 30)}
                fg="yellow"
                marker="braille"
              />
            </chart>
          </box>
          <box border="single" title="Traffic" height={10}>
            <barchart
              data={state.barData}
              barWidth={3}
              barGap={1}
              fg="cyan"
              valueFg="white"
              labelFg="gray"
            />
          </box>
        </layout>
      </box>
    </layout>
  );
}

function Tab2() {
  return (
    <layout direction="vertical">
      <box border="rounded" title="About" flex={1}>
        <text wrap={true} fg="cyan">
          {"Tela Demo — a port of ratatui's demo example.\n\nThis app demonstrates the native components available in Tela: box, text, layout, list, table, tabs, gauge, linegauge, sparkline, barchart, and chart.\n\nAll rendering happens in Rust via ratatui. All state management and view logic is pure JavaScript running in QuickJS.\n\nPress Tab to switch tabs, j/k to navigate lists, q to quit."}
        </text>
      </box>
    </layout>
  );
}

function view(state) {
  return (
    <box border="rounded" title="Tela Demo">
      <layout direction="vertical">
        <tabs height={1} selected={state.activeTab} highlight_fg="cyan" divider=" | ">
          <text>Gauges</text>
          <text>Network</text>
          <text>About</text>
        </tabs>
        {state.activeTab === 0 ? (
          <Tab0 state={state} flex={1} />
        ) : state.activeTab === 1 ? (
          <Tab1 state={state} flex={1} />
        ) : (
          <Tab2 flex={1} />
        )}
        <text height={1} fg="gray" align="center">
          Tab: switch tabs | j/k: navigate | q: quit
        </text>
      </layout>
    </box>
  );
}
