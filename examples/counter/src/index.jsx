var initialState = { count: 0 };

function reduce(state, action) {
  switch (action.type) {
    case "increment":
      return { count: state.count + 1 };
    case "decrement":
      return { count: state.count - 1 };
    default:
      return state;
  }
}

var keybindings = {
  normal: {
    j: "increment",
    k: "decrement",
    q: "quit",
  },
};

function view(state, dispatch) {
  return (
    <box border="rounded" title="Counter">
      <layout direction="vertical">
        <text align="center">
          Count: {state.count}
        </text>
        <text align="center" fg="gray">
          j: increment | k: decrement | q: quit
        </text>
      </layout>
    </box>
  );
}
