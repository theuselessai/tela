var NODES = [
  { id: "trigger_chat",  label: "trigger_chat",  type: "trigger",     category: "trigger" },
  { id: "deep_agent",    label: "deep_agent",     type: "deep_agent",  category: "executable" },
  { id: "switch_1",      label: "switch",          type: "switch",      category: "executable" },
  { id: "code_1",        label: "code",             type: "code",        category: "executable" },
  { id: "wait_1",        label: "wait",             type: "wait",        category: "executable" },
  { id: "merge_1",       label: "merge",            type: "merge",       category: "executable" },
  { id: "ai_model_1",    label: "ai_model",       type: "ai_model",    category: "sub", parent: "deep_agent" },
  { id: "mem_read",      label: "mem_read",       type: "memory_read", category: "sub", parent: "deep_agent" },
  { id: "mem_write",     label: "mem_write",      type: "memory_write",category: "sub", parent: "deep_agent" },
  { id: "skill_1",       label: "skill",            type: "skill",       category: "sub", parent: "deep_agent" },
];

var EDGES = [
  { source: "trigger_chat", target: "deep_agent",  label: "", edgeType: "direct" },
  { source: "deep_agent",   target: "switch_1",    label: "", edgeType: "direct" },
  { source: "switch_1",     target: "code_1",      label: "", edgeType: "conditional", condition: "A" },
  { source: "switch_1",     target: "wait_1",      label: "", edgeType: "conditional", condition: "B" },
  { source: "code_1",       target: "merge_1",     label: "", edgeType: "direct" },
  { source: "wait_1",       target: "merge_1",     label: "", edgeType: "direct" },
  { source: "ai_model_1",   target: "deep_agent",  label: "llm",   edgeType: "sub" },
  { source: "mem_read",     target: "deep_agent",  label: "tool",  edgeType: "sub" },
  { source: "mem_write",    target: "deep_agent",  label: "tool",  edgeType: "sub" },
  { source: "skill_1",      target: "deep_agent",  label: "skill", edgeType: "sub" },
];

var TYPE_COLORS = {
  trigger: "yellow", deep_agent: "magenta", agent: "magenta",
  switch: "cyan", loop: "cyan", merge: "cyan", wait: "cyan",
  code: "gray", ai_model: "blue", memory_read: "yellow",
  memory_write: "yellow", skill: "magenta", run_command: "green",
};

function nodeColor(type) { return TYPE_COLORS[type] || "white"; }

// Canvas y: 0=bottom, positive=up. Layer 0 (trigger) at top = highest y.
function layoutGraph(nodes, edges, canvasW, canvasH) {
  var mainW = 18;
  var mainH = 5;
  var subW = 12;
  var subH = 3;
  var vGap = 4;
  var branchHGap = 6;
  var subHGap = 3;
  var subVGap = 1;

  var triggers = [];
  var executables = [];
  var subs = {};
  for (var i = 0; i < nodes.length; i++) {
    var n = nodes[i];
    if (n.category === "trigger") triggers.push(n);
    else if (n.category === "executable") executables.push(n);
    else if (n.category === "sub") {
      if (!subs[n.parent]) subs[n.parent] = [];
      subs[n.parent].push(n);
    }
  }

  var branchTargets = {};
  for (var i = 0; i < edges.length; i++) {
    if (edges[i].edgeType === "conditional") branchTargets[edges[i].target] = edges[i];
  }
  var incomingCount = {};
  for (var i = 0; i < edges.length; i++) {
    var e = edges[i];
    if (e.edgeType === "direct" || e.edgeType === "conditional") {
      incomingCount[e.target] = (incomingCount[e.target] || 0) + 1;
    }
  }

  var positions = {};
  var spineX = canvasW / 2 - mainW / 2;
  var y = canvasH - mainH - 1;

  for (var i = 0; i < triggers.length; i++) {
    positions[triggers[i].id] = { x: spineX, y: y, w: mainW, h: mainH, cat: "trigger" };
    y -= (mainH + vGap);
  }

  var spineExec = [];
  var branchExec = [];
  for (var i = 0; i < executables.length; i++) {
    if (branchTargets[executables[i].id] && !(incomingCount[executables[i].id] > 1)) {
      branchExec.push(executables[i]);
    } else {
      spineExec.push(executables[i]);
    }
  }

  for (var i = 0; i < spineExec.length; i++) {
    var n = spineExec[i];
    positions[n.id] = { x: spineX, y: y, w: mainW, h: mainH, cat: "executable" };

    if (n.type === "switch") {
      y -= (mainH + vGap);
      var bIdx = 0;
      for (var b = 0; b < branchExec.length; b++) {
        var be = branchTargets[branchExec[b].id];
        if (be && be.source === n.id) {
          var bX = spineX + (bIdx === 0 ? -(mainW + branchHGap) : (mainW + branchHGap));
          positions[branchExec[b].id] = { x: bX, y: y, w: mainW, h: mainH, cat: "branch" };
          bIdx++;
        }
      }
      y -= (mainH + vGap);
    } else {
      y -= (mainH + vGap);
    }
  }

  for (var parentId in subs) {
    var pp = positions[parentId];
    if (!pp) continue;
    var subList = subs[parentId];
    var totalSubH = subList.length * subH + (subList.length - 1) * subVGap;
    var subStartY = pp.y + pp.h / 2 + totalSubH / 2 - subH;
    var subX = pp.x + pp.w + subHGap;
    for (var i = 0; i < subList.length; i++) {
      positions[subList[i].id] = {
        x: subX,
        y: subStartY - i * (subH + subVGap),
        w: subW,
        h: subH,
        cat: "sub",
      };
    }
  }

  return positions;
}

var initialState = {
  mode: "normal",
  selectedIndex: 0,
  panX: 0,
  panY: 0,
};

var keybindings = {
  normal: {
    q: "quit",
    j: "nav_next",
    k: "nav_prev",
    Tab: "nav_next",
    H: "pan_left",
    L: "pan_right",
    K: "pan_up",
    J: "pan_down",
    "0": "pan_reset",
  },
};

function reduce(state, action) {
  switch (action.type) {
    case "nav_next":
      return Object.assign({}, state, { selectedIndex: (state.selectedIndex + 1) % NODES.length });
    case "nav_prev":
      return Object.assign({}, state, { selectedIndex: (state.selectedIndex - 1 + NODES.length) % NODES.length });
    case "pan_left":
      return Object.assign({}, state, { panX: state.panX - 4 });
    case "pan_right":
      return Object.assign({}, state, { panX: state.panX + 4 });
    case "pan_up":
      return Object.assign({}, state, { panY: state.panY + 3 });
    case "pan_down":
      return Object.assign({}, state, { panY: state.panY - 3 });
    case "pan_reset":
      return Object.assign({}, state, { panX: 0, panY: 0 });
    default:
      return state;
  }
}

function view(state) {
  var cols = Tela.columns || 80;
  var rows = Tela.rows || 24;
  var graphRows = rows - 4;

  var positions = layoutGraph(NODES, EDGES, cols, graphRows);
  var selectedNode = NODES[state.selectedIndex];

  var xBounds = [state.panX, state.panX + cols - 2];
  var yBounds = [state.panY, state.panY + graphRows];

  var edgeEls = [];
  var labelEls = [];
  var nodeEls = [];

  for (var i = 0; i < EDGES.length; i++) {
    var e = EDGES[i];
    var sp = positions[e.source];
    var tp = positions[e.target];
    if (!sp || !tp) continue;

    if (e.edgeType === "sub") {
      var sx = sp.x;
      var sy = sp.y + sp.h / 2;
      var tx = tp.x + tp.w;
      var ty = tp.y + tp.h / 2;
      var midX = (sx + tx) / 2;
      var color = nodeColor(NODES.filter(function(n) { return n.id === e.source; })[0].type);
      edgeEls.push(
        <polyline key={"se" + i} points={[[sx, sy], [midX, sy], [midX, ty], [tx, ty]]} color={color} />
      );
      if (e.label) {
        labelEls.push(<text key={"sl" + i} x={midX + 0.5} y={sy + 0.8} color={color}>{e.label}</text>);
      }
    } else if (e.edgeType === "direct") {
      var sx = sp.x + sp.w / 2;
      var sy = sp.y;
      var tx = tp.x + tp.w / 2;
      var ty = tp.y + tp.h;
      edgeEls.push(<line key={"de" + i} x1={sx} y1={sy} x2={tx} y2={ty} color="darkgray" />);
    } else if (e.edgeType === "conditional") {
      var sx = sp.x + sp.w / 2;
      var sy = sp.y;
      var tx = tp.x + tp.w / 2;
      var ty = tp.y + tp.h;
      var midY = (sy + ty) / 2;
      edgeEls.push(
        <polyline key={"ce" + i} points={[[sx, sy], [sx, midY], [tx, midY], [tx, ty]]} color="white" />
      );
      if (e.condition) {
        var lx = tx > sx ? tx - 2 : tx + 1;
        labelEls.push(<text key={"cl" + i} x={lx} y={midY + 0.8} color="white">{e.condition}</text>);
      }
    }
  }

  for (var i = 0; i < NODES.length; i++) {
    var n = NODES[i];
    var p = positions[n.id];
    if (!p) continue;
    var isSelected = (i === state.selectedIndex);
    var color = isSelected ? "white" : nodeColor(n.type);

    nodeEls.push(
      <rect key={"nr" + i} x={p.x} y={p.y} width={p.w} height={p.h} color={color} />
    );
    nodeEls.push(
      <text key={"nt" + i} x={p.x + 1} y={p.y + p.h / 2} color={color}>{n.label}</text>
    );
  }

  var info = selectedNode
    ? selectedNode.label + " [" + selectedNode.type + "]" + (selectedNode.category === "sub" ? " > " + selectedNode.parent : "")
    : "";

  return (
    <layout direction="vertical">
      <box border="rounded" title=" Workflow Graph " borderStyle="gray" flex={1}>
        <canvas xBounds={xBounds} yBounds={yBounds} marker="braille">
          {edgeEls}
          {labelEls}
          {nodeEls}
        </canvas>
      </box>
      <text height={1} fg="gray">
        {"  j/k: navigate  HJKL: pan  0: reset  q: quit"}
      </text>
      <text height={1}>
        <span fg="cyan">{"  \u25B6 "}</span>
        <span fg="white">{info}</span>
      </text>
    </layout>
  );
}
