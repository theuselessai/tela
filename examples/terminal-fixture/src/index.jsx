// Synthetic parser fixture only: this does not start a shell or network session.
var terminal = Tela.terminal.create({ cols: 48, rows: 10 });
Tela.terminal.write(terminal, "\x1b[36mTela terminal fixture\x1b[m\r\n");
Tela.terminal.write(terminal, "\x1b[33mANSI colors\x1b[m, cursor, and alternate screen supported.\r\n");

var initialState = { mode: "terminal", prefix: false, sent: [] };
var keybindings = {
  terminal: {
    // A deliberate local escape prefix. All other keys reach terminal_key.
    "Ctrl+]": "terminal_prefix"
  }
};
function reduce(state, action) {
  if (action.type === "terminal_prefix") return Object.assign({}, state, { prefix: true });
  if (action.type === "terminal_key") {
    Tela.terminal.write(terminal, "key: " + action.key + " ctrl=" + action.ctrl + "\r\n");
    return Object.assign({}, state, { sent: state.sent.concat([action]) });
  }
  return state;
}
function view(state) {
  return <box border="rounded" title="Synthetic terminal (Ctrl+] is local)">
    <layout direction="vertical">
      <terminal id={terminal} focused={true} flex={1} />
    </layout>
  </box>;
}
