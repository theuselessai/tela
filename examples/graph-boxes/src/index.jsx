var NODES = [
  { id: "trigger",       label: "trigger_chat",     type: "trigger",          category: "trigger" },
  { id: "deep_agent",    label: "deep_agent",        type: "deep_agent",       category: "executable" },
  { id: "categorizer",   label: "categorizer",       type: "categorizer",      category: "executable" },
  { id: "switch",        label: "switch",             type: "switch",           category: "executable" },
  { id: "code",          label: "code",               type: "code",             category: "executable" },
  { id: "human",         label: "human_confirm",      type: "human_confirmation",category: "executable" },
  { id: "loop",          label: "loop",               type: "loop",             category: "executable" },
  { id: "agent_2",       label: "agent_2",            type: "agent",            category: "executable" },
  { id: "filter",        label: "filter",             type: "filter",           category: "executable" },
  { id: "subworkflow",   label: "subworkflow",        type: "workflow",         category: "executable" },
  { id: "merge",         label: "merge",              type: "merge",            category: "executable" },
  { id: "extractor",     label: "extractor",          type: "extractor",        category: "executable" },

  { id: "da_model",      label: "claude-opus",        type: "ai_model",         category: "sub", parent: "deep_agent" },
  { id: "da_cmd",        label: "run_command",         type: "run_command",      category: "sub", parent: "deep_agent" },
  { id: "da_memr",       label: "memory_read",         type: "memory_read",      category: "sub", parent: "deep_agent" },
  { id: "da_memw",       label: "memory_write",        type: "memory_write",     category: "sub", parent: "deep_agent" },
  { id: "da_platform",   label: "platform_api",        type: "platform_api",     category: "sub", parent: "deep_agent" },
  { id: "da_epic",       label: "epic_tools",          type: "epic_tools",       category: "sub", parent: "deep_agent" },
  { id: "da_spawn",      label: "spawn_and_await",     type: "spawn_and_await",  category: "sub", parent: "deep_agent" },
  { id: "da_skill",      label: "skill",               type: "skill",            category: "sub", parent: "deep_agent" },

  { id: "cat_model",     label: "gpt-4o",             type: "ai_model",         category: "sub", parent: "categorizer" },
  { id: "cat_parser",    label: "output_parser",       type: "output_parser",    category: "sub", parent: "categorizer" },

  { id: "a2_model",      label: "claude-sonnet",      type: "ai_model",         category: "sub", parent: "agent_2" },

  { id: "ext_model",     label: "gpt-4o-mini",        type: "ai_model",         category: "sub", parent: "extractor" },
  { id: "ext_parser",    label: "output_parser",       type: "output_parser",    category: "sub", parent: "extractor" },
];

var EDGES = [
  { source: "trigger",     target: "deep_agent",  edgeType: "direct" },
  { source: "deep_agent",  target: "categorizer", edgeType: "direct" },
  { source: "categorizer", target: "switch",      edgeType: "direct" },
  { source: "switch",      target: "code",        edgeType: "conditional", condition: "A" },
  { source: "switch",      target: "loop",        edgeType: "conditional", condition: "B" },
  { source: "switch",      target: "filter",      edgeType: "conditional", condition: "C" },
  { source: "code",        target: "human",       edgeType: "direct" },
  { source: "human",       target: "merge",       edgeType: "direct" },
  { source: "loop",        target: "agent_2",     edgeType: "direct", label: "loop_body" },
  { source: "agent_2",     target: "loop",        edgeType: "direct", label: "loop_return" },
  { source: "loop",        target: "merge",       edgeType: "direct" },
  { source: "filter",      target: "subworkflow", edgeType: "direct" },
  { source: "subworkflow", target: "merge",       edgeType: "direct" },
  { source: "merge",       target: "extractor",   edgeType: "direct" },

  { source: "da_model",    target: "deep_agent",  edgeType: "sub", label: "llm" },
  { source: "da_cmd",      target: "deep_agent",  edgeType: "sub", label: "tool" },
  { source: "da_memr",     target: "deep_agent",  edgeType: "sub", label: "tool" },
  { source: "da_memw",     target: "deep_agent",  edgeType: "sub", label: "tool" },
  { source: "da_platform", target: "deep_agent",  edgeType: "sub", label: "tool" },
  { source: "da_epic",     target: "deep_agent",  edgeType: "sub", label: "tool" },
  { source: "da_spawn",    target: "deep_agent",  edgeType: "sub", label: "tool" },
  { source: "da_skill",    target: "deep_agent",  edgeType: "sub", label: "skill" },

  { source: "cat_model",   target: "categorizer", edgeType: "sub", label: "llm" },
  { source: "cat_parser",  target: "categorizer", edgeType: "sub", label: "output_parser" },

  { source: "a2_model",    target: "agent_2",     edgeType: "sub", label: "llm" },

  { source: "ext_model",   target: "extractor",   edgeType: "sub", label: "llm" },
  { source: "ext_parser",  target: "extractor",   edgeType: "sub", label: "output_parser" },
];

var TYPE_COLORS = {
  trigger: "yellow", deep_agent: "magenta", agent: "magenta",
  categorizer: "magenta", router: "magenta", extractor: "magenta",
  switch: "blue", loop: "blue", merge: "blue", wait: "blue", filter: "blue",
  code: "gray", human_confirmation: "green", workflow: "cyan",
  ai_model: "cyan", memory_read: "yellow", memory_write: "yellow",
  run_command: "green", output_parser: "gray", skill: "magenta",
  platform_api: "green", epic_tools: "green", task_tools: "green",
  spawn_and_await: "green", scheduler_tools: "green",
};

function nodeColor(type) { return TYPE_COLORS[type] || "white"; }

function getSubTree(nodeId, nodes, edges) {
  var groups = {};
  for (var i = 0; i < edges.length; i++) {
    var e = edges[i];
    if (e.edgeType === "sub" && e.target === nodeId) {
      var label = e.label || "other";
      if (!groups[label]) groups[label] = [];
      var node = nodes.filter(function(n) { return n.id === e.source; })[0];
      if (node) groups[label].push(node);
    }
  }
  var lineCount = 0;
  var keys = Object.keys(groups);
  for (var i = 0; i < keys.length; i++) {
    lineCount += 1 + groups[keys[i]].length;
  }
  return { groups: groups, keys: keys, lineCount: lineCount };
}

function layoutGraph(nodes, edges, areaW, areaH) {
  var nodeH = 3;
  var nodeW = 24;
  var vGap = 2;
  var hGap = 4;

  var spineX = Math.floor(areaW / 2) - Math.floor(nodeW / 2);
  var branchLeftX = spineX - nodeW - hGap;
  var branchCenterX = spineX;
  var branchRightX = spineX + nodeW + hGap;

  var positions = {};
  var y = 0;

  function placeSpine(id, extraH) {
    var tree = getSubTree(id, nodes, edges);
    var h = Math.max(nodeH, (extraH || 0) + (tree.lineCount > 0 ? tree.lineCount + 4 : nodeH));
    positions[id] = { x: spineX, y: y, w: nodeW, h: h, cat: "executable", subTree: tree.lineCount > 0 ? tree : null };
    y += h + vGap;
  }

  positions["trigger"] = { x: spineX, y: y, w: nodeW, h: nodeH, cat: "trigger" };
  y += nodeH + vGap;

  placeSpine("deep_agent");
  placeSpine("categorizer");

  positions["switch"] = { x: spineX, y: y, w: nodeW, h: nodeH, cat: "executable" };
  y += nodeH + vGap;

  // 3 branches: A (left), B (center), C (right)
  var branchY = y;

  // Branch A: code → human_confirm
  positions["code"] = { x: branchLeftX, y: branchY, w: nodeW, h: nodeH, cat: "branch" };
  positions["human"] = { x: branchLeftX, y: branchY + nodeH + vGap, w: nodeW, h: nodeH, cat: "branch" };

  // Branch B: loop → agent_2 (loop)
  var a2Tree = getSubTree("agent_2", nodes, edges);
  var a2H = Math.max(nodeH, a2Tree.lineCount > 0 ? a2Tree.lineCount + 4 : nodeH);
  positions["loop"] = { x: branchCenterX, y: branchY, w: nodeW, h: nodeH, cat: "branch" };
  positions["agent_2"] = { x: branchCenterX, y: branchY + nodeH + vGap, w: nodeW, h: a2H, cat: "branch", subTree: a2Tree.lineCount > 0 ? a2Tree : null };

  // Branch C: filter → subworkflow
  positions["filter"] = { x: branchRightX, y: branchY, w: nodeW, h: nodeH, cat: "branch" };
  positions["subworkflow"] = { x: branchRightX, y: branchY + nodeH + vGap, w: nodeW, h: nodeH, cat: "branch" };

  var maxBranchBottom = 0;
  var branchIds = ["human", "agent_2", "subworkflow"];
  for (var i = 0; i < branchIds.length; i++) {
    var bp = positions[branchIds[i]];
    var bottom = bp.y + bp.h;
    if (bottom > maxBranchBottom) maxBranchBottom = bottom;
  }
  y = maxBranchBottom + vGap;

  positions["merge"] = { x: spineX, y: y, w: nodeW, h: nodeH, cat: "executable" };
  y += nodeH + vGap;

  placeSpine("extractor");

  return positions;
}

function drawVLine(els, x, y1, y2, color, prefix) {
  for (var y = y1; y <= y2; y++) {
    els.push(<text key={prefix + y} x={x} y={y} width={1} height={1} fg={color}>{"\u2502"}</text>);
  }
}

var initialState = { mode: "normal", selectedIndex: 0, panX: 0, panY: 0 };

var keybindings = {
  normal: {
    q: "quit", j: "nav_next", k: "nav_prev", Tab: "nav_next",
    H: "pan_left", L: "pan_right", K: "pan_up", J: "pan_down", "0": "pan_reset",
  },
};

function reduce(state, action) {
  switch (action.type) {
    case "nav_next":
      return Object.assign({}, state, { selectedIndex: (state.selectedIndex + 1) % NODES.length });
    case "nav_prev":
      return Object.assign({}, state, { selectedIndex: (state.selectedIndex - 1 + NODES.length) % NODES.length });
    case "pan_left":  return Object.assign({}, state, { panX: state.panX - 4 });
    case "pan_right": return Object.assign({}, state, { panX: state.panX + 4 });
    case "pan_up":    return Object.assign({}, state, { panY: Math.max(0, state.panY - 2) });
    case "pan_down":  return Object.assign({}, state, { panY: state.panY + 2 });
    case "pan_reset": return Object.assign({}, state, { panX: 0, panY: 0 });
    default: return state;
  }
}

function view(state) {
  var cols = Tela.columns || 80;
  var rows = Tela.rows || 24;
  var graphH = rows - 4;
  var graphW = cols - 2;

  var pos = layoutGraph(NODES, EDGES, graphW, graphH);
  var selectedNode = NODES[state.selectedIndex];
  var edgeEls = [];
  var nodeEls = [];
  var px = state.panX;
  var py = state.panY;
  var p = function(id) { return pos[id]; };

  function center(id) { return p(id).x + Math.floor(p(id).w / 2) - px; }
  function bot(id) { return p(id).y + p(id).h - py; }
  function top_(id) { return p(id).y - py; }
  function right_(id) { return p(id).x + p(id).w - px; }
  function midY(id) { return p(id).y + Math.floor(p(id).h / 2) - py; }

  var spineC = center("trigger");

  // Spine connectors: trigger → deep_agent → categorizer → switch
  var spineFlow = [["trigger","deep_agent"],["deep_agent","categorizer"],["categorizer","switch"]];
  for (var i = 0; i < spineFlow.length; i++) {
    drawVLine(edgeEls, spineC, bot(spineFlow[i][0]), top_(spineFlow[i][1]) - 1, "darkgray", "sp_" + i + "_");
  }

  // Fork from switch
  var leftC = center("code");
  var centerC = center("loop");
  var rightC = center("filter");
  var forkY = bot("switch");

  edgeEls.push(<text key="fv" x={spineC} y={forkY} width={1} height={1} fg="white">{"\u2502"}</text>);
  var forkBarY = forkY + 1;
  for (var x = leftC; x <= rightC; x++) {
    var ch = "\u2500";
    if (x === leftC) ch = "\u250C";
    else if (x === rightC) ch = "\u2510";
    else if (x === spineC) ch = "\u2534";
    else if (x === centerC && centerC !== spineC) ch = "\u252C";
    edgeEls.push(<text key={"fh_" + x} x={x} y={forkBarY} width={1} height={1} fg="white">{ch}</text>);
  }

  edgeEls.push(<text key="la" x={leftC + 1} y={forkBarY} width={1} height={1} fg="white">{"A"}</text>);
  edgeEls.push(<text key="lb" x={centerC + 1} y={forkBarY} width={1} height={1} fg="white">{"B"}</text>);
  edgeEls.push(<text key="lc" x={rightC - 2} y={forkBarY} width={1} height={1} fg="white">{"C"}</text>);

  drawVLine(edgeEls, leftC, forkBarY + 1, top_("code") - 1, "white", "fl_");
  drawVLine(edgeEls, centerC, forkBarY + 1, top_("loop") - 1, "white", "fc_");
  drawVLine(edgeEls, rightC, forkBarY + 1, top_("filter") - 1, "white", "fr_");

  // Branch A: code → human
  drawVLine(edgeEls, leftC, bot("code"), top_("human") - 1, "darkgray", "e_ch_");

  // Branch B: loop → agent_2 + loop_return
  drawVLine(edgeEls, centerC, bot("loop"), top_("agent_2") - 1, "darkgray", "e_la_");
  var backX = right_("loop") + 3;
  // top: ◀──╮
  for (var x = right_("loop"); x <= backX; x++) {
    var ch = (x === right_("loop")) ? "\u25C2" : (x === backX) ? "\u256E" : "\u2500";
    edgeEls.push(<text key={"lr_t" + x} x={x} y={midY("loop")} width={1} height={1} fg="blue">{ch}</text>);
  }
  drawVLine(edgeEls, backX, midY("loop") + 1, midY("agent_2") - 1, "blue", "lr_v_");
  // bottom: ──╯
  for (var x = right_("agent_2"); x <= backX; x++) {
    var ch = (x === backX) ? "\u256F" : "\u2500";
    edgeEls.push(<text key={"lr_b" + x} x={x} y={midY("agent_2")} width={1} height={1} fg="blue">{ch}</text>);
  }

  // Branch C: filter → subworkflow
  drawVLine(edgeEls, rightC, bot("filter"), top_("subworkflow") - 1, "darkgray", "e_fs_");

  // Merge bar
  var mergeBarY = top_("merge") - 1;
  var humanBot = bot("human");
  var a2Bot = bot("agent_2");
  var swBot = bot("subworkflow");
  drawVLine(edgeEls, leftC, humanBot, mergeBarY - 1, "darkgray", "e_hm_");
  drawVLine(edgeEls, centerC, a2Bot + 1, mergeBarY - 1, "darkgray", "e_lm_");
  drawVLine(edgeEls, rightC, swBot, mergeBarY - 1, "darkgray", "e_sm_");

  for (var x = leftC; x <= rightC; x++) {
    var ch = "\u2500";
    if (x === leftC) ch = "\u2514";
    else if (x === rightC) ch = "\u2518";
    else if (x === spineC) ch = "\u252C";
    else if (x === centerC && centerC !== spineC) ch = "\u2534";
    edgeEls.push(<text key={"mh_" + x} x={x} y={mergeBarY} width={1} height={1} fg="darkgray">{ch}</text>);
  }

  // merge → extractor
  drawVLine(edgeEls, spineC, bot("merge"), top_("extractor") - 1, "darkgray", "e_me_");

  // --- Node rendering ---
  for (var i = 0; i < NODES.length; i++) {
    var n = NODES[i];
    if (n.category === "sub") continue;
    var np = pos[n.id];
    if (!np) continue;
    var npx = np.x - px;
    var npy = np.y - py;
    var isSelected = (i === state.selectedIndex);
    if (npx + np.w < 0 || npy + np.h < 0 || npx >= graphW || npy >= graphH) continue;

    var border = isSelected ? "double" : (np.cat === "trigger" ? "double" : "rounded");
    var bStyle = isSelected ? "white" : nodeColor(n.type);

    if (np.subTree) {
      var tree = np.subTree;
      var lines = [];
      lines.push({ text: n.type, color: nodeColor(n.type), dim: !isSelected });
      lines.push({ text: "", color: "gray" });
      for (var g = 0; g < tree.keys.length; g++) {
        var groupNodes = tree.groups[tree.keys[g]];
        lines.push({ text: " \u25CF " + tree.keys[g], color: "gray" });
        for (var s = 0; s < groupNodes.length; s++) {
          var isLast = (s === groupNodes.length - 1);
          var conn = isLast ? " \u2570\u2500 " : " \u251C\u2500 ";
          lines.push({ text: "  " + conn + groupNodes[s].label, color: nodeColor(groupNodes[s].type) });
        }
      }
      nodeEls.push(
        <box key={"n" + i} x={npx} y={npy} width={np.w} height={np.h}
          border={border} borderStyle={bStyle}
          title={" " + n.label + " "} titleAlignment="left">
          <layout direction="vertical">
            {lines.map(function(l, li) {
              return <text key={"tl" + li} height={1} fg={l.color} dim={l.dim || false}>{l.text}</text>;
            })}
          </layout>
        </box>
      );
    } else {
      nodeEls.push(
        <box key={"n" + i} x={npx} y={npy} width={np.w} height={np.h}
          border={border} borderStyle={bStyle}
          title={" " + n.label + " "} titleAlignment="left">
          <text fg={nodeColor(n.type)} dim={!isSelected}>{n.type}</text>
        </box>
      );
    }
  }

  var info = selectedNode
    ? selectedNode.label + " [" + selectedNode.type + "]" + (selectedNode.category === "sub" ? " > " + selectedNode.parent : "")
    : "";

  return (
    <layout direction="vertical">
      <box border="rounded" title=" Workflow Graph " borderStyle="gray" flex={1}>
        <layout position="absolute">
          {edgeEls}
          {nodeEls}
        </layout>
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
