var TASKS = [
  "Item1", "Item2", "Item3", "Item4", "Item5", "Item6", "Item7", "Item8",
  "Item9", "Item10", "Item11", "Item12", "Item13", "Item14", "Item15", "Item16",
  "Item17", "Item18", "Item19", "Item20", "Item21", "Item22", "Item23", "Item24",
];

var LOGS = [
  ["Event1", "INFO"], ["Event2", "INFO"], ["Event3", "CRITICAL"],
  ["Event4", "ERROR"], ["Event5", "INFO"], ["Event6", "INFO"],
  ["Event7", "WARNING"], ["Event8", "INFO"], ["Event9", "INFO"],
  ["Event10", "INFO"], ["Event11", "CRITICAL"], ["Event12", "INFO"],
  ["Event13", "INFO"], ["Event14", "INFO"], ["Event15", "INFO"],
  ["Event16", "INFO"], ["Event17", "ERROR"], ["Event18", "ERROR"],
  ["Event19", "INFO"], ["Event20", "INFO"], ["Event21", "WARNING"],
  ["Event22", "INFO"], ["Event23", "INFO"], ["Event24", "WARNING"],
  ["Event25", "INFO"], ["Event26", "INFO"],
];

var EVENTS = [
  ["B1", 9], ["B2", 12], ["B3", 5], ["B4", 8], ["B5", 2], ["B6", 4],
  ["B7", 5], ["B8", 9], ["B9", 14], ["B10", 15], ["B11", 1], ["B12", 0],
  ["B13", 4], ["B14", 6], ["B15", 4], ["B16", 6], ["B17", 4], ["B18", 7],
  ["B19", 13], ["B20", 8], ["B21", 11], ["B22", 9], ["B23", 3], ["B24", 5],
];

var SERVERS = [
  { name: "NorthAmerica-1", location: "New York City", status: "Up" },
  { name: "Europe-1", location: "Paris", status: "Failure" },
  { name: "SouthAmerica-1", location: "São Paulo", status: "Up" },
  { name: "Asia-1", location: "Singapore", status: "Up" },
];

var COLORS = [
  "Reset", "Black", "Red", "Green", "Yellow", "Blue", "Magenta", "Cyan",
  "Gray", "DarkGray", "LightRed", "LightGreen", "LightYellow", "LightBlue",
  "LightMagenta", "LightCyan", "White",
];

var COLOR_VALUES = [
  "reset", "black", "red", "green", "yellow", "blue", "magenta", "cyan",
  "gray", "darkgray", "lightred", "lightgreen", "lightyellow", "lightblue",
  "lightmagenta", "lightcyan", "white",
];

function randomSignal(lower, upper, count) {
  var arr = [];
  for (var i = 0; i < count; i++) {
    arr.push(Math.floor(Math.random() * (upper - lower)) + lower);
  }
  return arr;
}

function sinSignal(interval, period, scale, count) {
  var pts = [];
  for (var i = 0; i < count; i++) {
    var x = i * interval;
    pts.push([x, Math.sin(x / period) * scale]);
  }
  return pts;
}

var initialState = {
  mode: "normal",
  activeTab: 0,
  taskIndex: 0,
  showChart: true,
  progress: 0.0,
  tick: 0,
  sparkData: randomSignal(0, 100, 300),
  logs: LOGS.slice(),
  barData: EVENTS.slice(),
  sin1: sinSignal(0.2, 3.0, 18.0, 100),
  sin2: sinSignal(0.1, 2.0, 10.0, 200),
  sinWindow: [0.0, 20.0],
  sin1X: 100 * 0.2,
  sin2X: 200 * 0.1,
};

function reduce(state, action) {
  switch (action.type) {
    case "tick": {
      var newProgress = state.progress + 0.001;
      if (newProgress > 1.0) newProgress = 0.0;

      var newSparkData = state.sparkData.slice(1);
      newSparkData.push(Math.floor(Math.random() * 100));

      var newLogs = state.logs.slice();
      var lastLog = newLogs.pop();
      newLogs.unshift(lastLog);

      var newBarData = state.barData.slice();
      var lastBar = newBarData.pop();
      newBarData.unshift(lastBar);

      var newSin1 = state.sin1.slice(5);
      var s1x = state.sin1X;
      for (var i = 0; i < 5; i++) {
        newSin1.push([s1x, Math.sin(s1x / 3.0) * 18.0]);
        s1x += 0.2;
      }

      var newSin2 = state.sin2.slice(10);
      var s2x = state.sin2X;
      for (var j = 0; j < 10; j++) {
        newSin2.push([s2x, Math.sin(s2x / 2.0) * 10.0]);
        s2x += 0.1;
      }

      return Object.assign({}, state, {
        tick: state.tick + 1,
        progress: newProgress,
        sparkData: newSparkData,
        logs: newLogs,
        barData: newBarData,
        sin1: newSin1,
        sin2: newSin2,
        sinWindow: [state.sinWindow[0] + 1.0, state.sinWindow[1] + 1.0],
        sin1X: s1x,
        sin2X: s2x,
      });
    }
    case "next_tab":
      return Object.assign({}, state, { activeTab: (state.activeTab + 1) % 3 });
    case "prev_tab":
      return Object.assign({}, state, { activeTab: (state.activeTab + 2) % 3 });
    case "move_down":
      return Object.assign({}, state, {
        taskIndex: state.taskIndex >= TASKS.length - 1 ? 0 : state.taskIndex + 1,
      });
    case "move_up":
      return Object.assign({}, state, {
        taskIndex: state.taskIndex === 0 ? TASKS.length - 1 : state.taskIndex - 1,
      });
    case "toggle_chart":
      return Object.assign({}, state, { showChart: !state.showChart });
    default:
      return state;
  }
}

var keybindings = {
  normal: {
    h: "prev_tab",
    l: "next_tab",
    Left: "prev_tab",
    Right: "next_tab",
    j: "move_down",
    k: "move_up",
    Down: "move_down",
    Up: "move_up",
    t: "toggle_chart",
    q: "quit",
  },
};

setInterval(function() {
  Tela.dispatch({ type: "tick" });
}, 250);

function DrawGauges({ state }) {
  var pctLabel = (state.progress * 100).toFixed(2) + "%";
  return (
    <box border="single" title="Graphs" height={9}>
      <layout direction="vertical">
        <gauge height={2} percent={Math.round(state.progress * 100)} label={"Gauge: " + pctLabel} fg="magenta" />
        <sparkline height={3} data={state.sparkData} fg="green" />
        <linegauge height={2} ratio={state.progress} label={"LineGauge: " + pctLabel} fg="magenta" />
      </layout>
    </box>
  );
}

function DrawCharts({ state }) {
  return (
    <layout direction="horizontal" flex={1}>
      <layout direction="vertical" percent={50}>
        <layout direction="horizontal" percent={50}>
          <box border="single" title="List" flex={1}>
            <list selected={state.taskIndex} highlight_symbol="> ">
              {TASKS.map(function(t, i) { return <text key={i}>{t}</text>; })}
            </list>
          </box>
          <box border="single" title="List" flex={1}>
            <list>
              {state.logs.map(function(log, i) {
                var evt = log[0];
                var level = log[1];
                var color = level === "ERROR" ? "magenta"
                  : level === "CRITICAL" ? "red"
                  : level === "WARNING" ? "yellow"
                  : "blue";
                return (
                  <text key={i}>
                    <span fg={color}>{level.padEnd(9)}</span>
                    <span>{evt}</span>
                  </text>
                );
              })}
            </list>
          </box>
        </layout>
        <box border="single" title="Bar chart" percent={50}>
          <barchart data={state.barData} barWidth={3} barGap={2} fg="green" valueFg="green" labelFg="yellow" />
        </box>
      </layout>
      {state.showChart ? (
        <box border="single" title="Chart" percent={50}>
          <chart
            xBounds={state.sinWindow}
            yBounds={[-20, 20]}
            xLabels={[
              String(state.sinWindow[0]),
              String((state.sinWindow[0] + state.sinWindow[1]) / 2),
              String(state.sinWindow[1]),
            ]}
            yLabels={["-20", "0", "20"]}
            xTitle="X Axis"
            yTitle="Y Axis"
          >
            <dataset name="data2" data={state.sin1} fg="cyan" marker="dot" />
            <dataset name="data3" data={state.sin2} fg="yellow" marker="braille" />
          </chart>
        </box>
      ) : null}
    </layout>
  );
}

function DrawFooter() {
  return (
    <box border="single" title="Footer" height={7}>
      <text wrap="trim">
        {"This is a paragraph with several lines. You can change style your text the way you want\n\n"}
        <span>{"For example: "}</span>
        <span fg="red">under</span>
        <span> </span>
        <span fg="green">the</span>
        <span> </span>
        <span fg="blue">rainbow</span>
        <span>.</span>
        {"\n"}
        <span>{"Oh and if you didn't "}</span>
        <span italic={true}>notice</span>
        <span>{" you can "}</span>
        <span bold={true}>automatically</span>
        <span> </span>
        <span bold={true}>wrap</span>
        <span> your </span>
        <span underline={true}>text</span>
        <span>.</span>
        {"\nOne more thing is that it should display unicode characters: 10€"}
      </text>
    </box>
  );
}

function DrawTab0({ state }) {
  return (
    <layout direction="vertical" flex={1}>
      <DrawGauges state={state} />
      <DrawCharts state={state} />
      <DrawFooter />
    </layout>
  );
}

function DrawTab1({ state }) {
  return (
    <layout direction="horizontal" flex={1}>
      <box border="single" title="Servers" width={45}>
        <table header={["Server", "Location", "Status"]} widths={[15, 15, 10]}>
          {SERVERS.map(function(s, i) {
            var color = s.status === "Up" ? "green" : "red";
            return (
              <row key={i}>
                <text fg={color}>{s.name}</text>
                <text fg={color}>{s.location}</text>
                <text fg={color}>{s.status}</text>
              </row>
            );
          })}
        </table>
      </box>
      <box border="single" title="Network" flex={1}>
        <layout direction="vertical">
          <chart
            flex={1}
            xBounds={state.sinWindow}
            yBounds={[-20, 20]}
            xLabels={[
              String(state.sinWindow[0]),
              String((state.sinWindow[0] + state.sinWindow[1]) / 2),
              String(state.sinWindow[1]),
            ]}
            yLabels={["-20", "0", "20"]}
            xTitle="X Axis"
            yTitle="Y Axis"
          >
            <dataset name="sin1" data={state.sin1} fg="cyan" marker="braille" />
            <dataset name="sin2" data={state.sin2} fg="yellow" marker="braille" />
          </chart>
          <barchart height={8} data={state.barData} barWidth={3} barGap={2} fg="green" valueFg="green" labelFg="yellow" />
        </layout>
      </box>
    </layout>
  );
}

function DrawTab2() {
  return (
    <layout direction="horizontal" flex={1}>
      <box border="single" title="Colors" flex={1}>
        <table widths={[20, 15, 15]}>
          {COLORS.map(function(name, i) {
            var c = COLOR_VALUES[i];
            return (
              <row key={i}>
                <text>{name + ": "}</text>
                <text fg={c}>Foreground</text>
                <text bg={c}>Background</text>
              </row>
            );
          })}
        </table>
      </box>
      <layout flex={1} />
    </layout>
  );
}

function view(state) {
  return (
    <layout direction="vertical">
      <box border="single" title="Crossterm Demo" height={3}>
        <tabs selected={state.activeTab} highlight_fg="yellow">
          {["Tab0", "Tab1", "Tab2"].map(function(t, i) {
            return <text key={i} fg="green">{t}</text>;
          })}
        </tabs>
      </box>
      {state.activeTab === 0 ? (
        <DrawTab0 state={state} flex={1} />
      ) : state.activeTab === 1 ? (
        <DrawTab1 state={state} flex={1} />
      ) : (
        <DrawTab2 flex={1} />
      )}
    </layout>
  );
}
