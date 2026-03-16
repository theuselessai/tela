var initialState = {
  activeTab: 0,
  selectedIndex: 0,
  todos: [
    { text: "Learn Tela framework", done: true },
    { text: "Build a TUI app", done: false },
    { text: "Add more components", done: false }
  ],
  nextId: 4
};

function reduce(state, action) {
  switch (action.type) {
    case "move_down":
      return Object.assign({}, state, {
        selectedIndex: Math.min(state.selectedIndex + 1, state.todos.length - 1)
      });
    case "move_up":
      return Object.assign({}, state, {
        selectedIndex: Math.max(state.selectedIndex - 1, 0)
      });
    case "toggle": {
      var todos = state.todos.map(function(t, i) {
        if (i === state.selectedIndex) {
          return Object.assign({}, t, { done: !t.done });
        }
        return t;
      });
      return Object.assign({}, state, { todos: todos });
    }
    case "next_tab":
      return Object.assign({}, state, {
        activeTab: (state.activeTab + 1) % 2
      });
    case "add_todo": {
      var newTodo = { text: "Todo " + state.nextId, done: false };
      return Object.assign({}, state, {
        todos: state.todos.concat([newTodo]),
        nextId: state.nextId + 1
      });
    }
    case "delete_todo": {
      if (state.todos.length === 0) return state;
      var filtered = state.todos.filter(function(_, i) {
        return i !== state.selectedIndex;
      });
      var newIndex = Math.min(state.selectedIndex, Math.max(filtered.length - 1, 0));
      return Object.assign({}, state, {
        todos: filtered,
        selectedIndex: newIndex
      });
    }
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
    a: "add_todo",
    d: "delete_todo",
    q: "quit"
  }
};

function view(state) {
  var doneCount = state.todos.filter(function(t) { return t.done; }).length;
  var totalCount = state.todos.length;
  var percent = totalCount > 0 ? Math.round((doneCount / totalCount) * 100) : 0;

  return (
    <box border="rounded" title="Todo List">
      <layout direction="vertical">
        <tabs height={1} selected={state.activeTab} highlight_fg="cyan">
          <text>Tasks</text>
          <text>Stats</text>
        </tabs>
        {state.activeTab === 0 ? (
          <layout direction="vertical" flex={1}>
            <list flex={1} selected={state.selectedIndex} highlight_fg="yellow" highlight_symbol="> ">
              {state.todos.map(function(todo, i) {
                return (
                  <text key={i}>
                    <span fg={todo.done ? "green" : "red"}>{todo.done ? "[x] " : "[ ] "}</span>
                    <span italic={todo.done}>{todo.text}</span>
                  </text>
                );
              })}
            </list>
            <input height={1} value="" placeholder="Press 'a' to add a todo" fg="white" />
            <text height={1} fg="gray" align="center">
              j/k: navigate | space: toggle | a: add | d: delete | t: tabs | q: quit
            </text>
          </layout>
        ) : (
          <layout direction="vertical" flex={1}>
            <text height={1} align="center" bold={true}>
              {"Completion: " + doneCount + "/" + totalCount}
            </text>
            <gauge height={1} percent={percent} label={percent + "% complete"} fg="green" />
            <table flex={1} header={["#", "Task", "Status"]} widths={[5, 30, 10]}>
              {state.todos.map(function(todo, i) {
                return (
                  <row key={i}>
                    <text>{String(i + 1)}</text>
                    <text>{todo.text}</text>
                    <text>{todo.done ? "Done" : "Pending"}</text>
                  </row>
                );
              })}
            </table>
            <text height={1} fg="gray" align="center">
              t: switch tabs | q: quit
            </text>
          </layout>
        )}
      </layout>
    </box>
  );
}
