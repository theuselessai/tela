var initialState = {
  mode: "normal",
  activeTab: 0,
  selectedIndex: 0,
  input: "",
  count: 0,
  items: [
    { text: "Item A", done: false },
    { text: "Item B", done: true },
    { text: "Item C", done: false }
  ]
};

function reduce(state, action) {
  switch (action.type) {
    case "move_down":
      return Object.assign({}, state, {
        selectedIndex: Math.min(state.selectedIndex + 1, state.items.length - 1)
      });
    case "move_up":
      return Object.assign({}, state, {
        selectedIndex: Math.max(state.selectedIndex - 1, 0)
      });
    case "toggle":
      var items = state.items.map(function(item, i) {
        if (i === state.selectedIndex) {
          return Object.assign({}, item, { done: !item.done });
        }
        return item;
      });
      return Object.assign({}, state, { items: items });
    case "next_tab":
      return Object.assign({}, state, {
        activeTab: (state.activeTab + 1) % 3
      });
    case "increment":
      return Object.assign({}, state, { count: state.count + 1 });
    case "decrement":
      return Object.assign({}, state, { count: state.count - 1 });
    case "enter_insert":
      return Object.assign({}, state, { mode: "insert" });
    case "enter_normal":
      return Object.assign({}, state, { mode: "normal" });
    case "input_char":
      return Object.assign({}, state, { input: state.input + action.char });
    case "input_backspace":
      return Object.assign({}, state, { input: state.input.slice(0, -1) });
    case "input_submit":
      if (state.input.length > 0) {
        var newItem = { text: state.input, done: false };
        return Object.assign({}, state, {
          items: state.items.concat([newItem]),
          input: "",
          mode: "normal"
        });
      }
      return Object.assign({}, state, { mode: "normal" });
    default:
      return state;
  }
}

var keybindings = {
  normal: {
    j: "move_down",
    k: "move_up",
    " ": "toggle",
    t: "next_tab",
    "+": "increment",
    "-": "decrement",
    i: "enter_insert",
    q: "quit"
  },
  insert: {}
};

function TabList({ items, selectedIndex }) {
  return (
    <list selected={selectedIndex} highlight_fg="yellow" highlight_symbol="> ">
      {items.map(function(item, i) {
        return (
          <text key={i}>
            <span fg={item.done ? "green" : "red"}>{item.done ? "[x] " : "[ ] "}</span>
            <span italic={item.done}>{item.text}</span>
          </text>
        );
      })}
    </list>
  );
}

function TabCounter({ count }) {
  return (
    <layout direction="vertical">
      <text flex={1} align="center" bold={true}>
        {"Count: " + count}
      </text>
      <text height={1} fg="gray" align="center">+: increment | -: decrement</text>
    </layout>
  );
}

function TabTable({ items }) {
  return (
    <table header={["#", "Item", "Status"]} widths={[5, 30, 10]}>
      {items.map(function(item, i) {
        return (
          <row key={i}>
            <text>{String(i + 1)}</text>
            <text>{item.text}</text>
            <text fg={item.done ? "green" : "yellow"}>{item.done ? "Done" : "Pending"}</text>
          </row>
        );
      })}
    </table>
  );
}

function view(state) {
  var doneCount = state.items.filter(function(i) { return i.done; }).length;
  var percent = state.items.length > 0 ? Math.round((doneCount / state.items.length) * 100) : 0;

  return (
    <box border="rounded" title={"Test All [" + state.mode + "]"}>
      <layout direction="vertical">
        <tabs height={1} selected={state.activeTab} highlight_fg="cyan">
          <text>List</text>
          <text>Counter</text>
          <text>Table</text>
        </tabs>
        {state.activeTab === 0 ? (
          <layout direction="vertical" flex={1}>
            <TabList items={state.items} selectedIndex={state.selectedIndex} />
            <input height={1} value={state.input} placeholder="Press 'i' to add item" fg="white" />
          </layout>
        ) : state.activeTab === 1 ? (
          <TabCounter count={state.count} flex={1} />
        ) : (
          <TabTable items={state.items} flex={1} />
        )}
        <gauge height={1} percent={percent} label={percent + "% done"} fg="green" />
        <text height={1} fg="gray" align="center">
          {"j/k: nav | space: toggle | t: tab | +/-: count | i: insert | q: quit"}
        </text>
      </layout>
    </box>
  );
}
