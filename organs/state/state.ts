#!/usr/bin/env node
import * as fs from "node:fs";
import * as path from "node:path";
import * as process from "node:process";

function parseArgs(): Record<string, string> {
  const args: Record<string, string> = {};
  const argv = process.argv.slice(2);
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (arg.startsWith("--")) {
      const key = arg.slice(2);
      if (i + 1 < argv.length && !argv[i + 1].startsWith("--")) {
        args[key] = argv[++i];
      } else {
        args[key] = "true";
      }
    }
  }
  return args;
}

function findWorkspaceRoot(): string {
  const envRoot = process.env.PRESENCE_WORKSPACE;
  if (envRoot && fs.existsSync(envRoot) && fs.statSync(envRoot).isDirectory()) {
    return envRoot;
  }
  let current = path.resolve(import.meta.dirname);
  while (path.dirname(current) !== current) {
    if (
      fs.existsSync(path.join(current, "memory")) ||
      fs.existsSync(path.join(current, "AGENTS.md"))
    ) {
      return current;
    }
    current = path.dirname(current);
  }
  return process.cwd();
}

function getActiveAgentCard(): string {
  const root = findWorkspaceRoot();
  const agentName = process.env.PRESENCE_AGENT || "arche";
  const card = path.join(root, "agents", `${agentName}.agent.md`);
  if (fs.existsSync(card)) {
    return card;
  }
  const fallback = path.join(root, "agents", "arche.agent.md");
  if (fs.existsSync(fallback)) {
    return fallback;
  }
  return card;
}

function getSnapshotsDir(): string {
  const d = path.join(findWorkspaceRoot(), "memory", "snapshots");
  fs.mkdirSync(d, { recursive: true });
  return d;
}

interface AgentCardData {
  state: Record<string, string>;
  organs: string[];
  [key: string]: unknown;
}

function parseAgentFrontmatter(cardPath: string): { data: AgentCardData; fmRaw: string; body: string } {
  if (!fs.existsSync(cardPath)) {
    return { data: { state: {}, organs: [] }, fmRaw: "", body: "" };
  }
  const text = fs.readFileSync(cardPath, "utf8").replace(/^\uFEFF/, "");
  const m = text.match(/^---\r?\n([\s\S]*?)\r?\n---\r?\n([\s\S]*)$/);
  if (!m) {
    return { data: { state: {}, organs: [] }, fmRaw: "", body: text };
  }
  const fmRaw = m[1];
  const body = m[2];
  const data: Record<string, unknown> = {};
  let inState = false;
  let inOrgans = false;
  const stateDict: Record<string, string> = {};
  const organsList: string[] = [];

  for (const line of fmRaw.split(/\r?\n/)) {
    if (line.trim() === "state:" || line.startsWith("state:")) {
      inState = true;
      inOrgans = false;
      continue;
    }
    if (line.trim() === "organs:" || line.startsWith("organs:")) {
      inOrgans = true;
      inState = false;
      continue;
    }
    if (inState) {
      const sm = line.match(/^\s+([a-zA-Z0-9_]+):\s*(.*)$/);
      if (sm) {
        stateDict[sm[1]] = sm[2].trim();
        continue;
      } else if (/^[a-zA-Z0-9_]+:/.test(line)) {
        inState = false;
      }
    }
    if (inOrgans) {
      const om = line.match(/^\s*-\s*([a-zA-Z0-9_-]+)/);
      if (om) {
        organsList.push(om[1]);
        continue;
      } else if (/^[a-zA-Z0-9_]+:/.test(line)) {
        inOrgans = false;
      }
    }

    const mTop = line.match(/^([a-zA-Z0-9_]+):\s*(.*)$/);
    if (mTop) {
      data[mTop[1]] = mTop[2].trim();
    }
  }

  data.state = stateDict;
  data.organs = organsList;
  return { data: data as AgentCardData, fmRaw, body };
}

function updateAgentCardState(cardPath: string, updates: Record<string, string>): boolean {
  if (!fs.existsSync(cardPath)) {
    return false;
  }
  const text = fs.readFileSync(cardPath, "utf8").replace(/^\uFEFF/, "");
  const m = text.match(/^---\r?\n([\s\S]*?)\r?\n---\r?\n([\s\S]*)$/);
  if (!m) {
    return false;
  }
  const fmRaw = m[1];
  const body = m[2];

  const lines = fmRaw.split(/\r?\n/);
  const newLines: string[] = [];
  let inState = false;
  const stateKeysWritten = new Set<string>();
  let hasStateBlock = false;

  for (const line of lines) {
    if (line.trim() === "state:" || line.startsWith("state:")) {
      inState = true;
      hasStateBlock = true;
      newLines.push("state:");
      continue;
    }
    if (inState) {
      const sm = line.match(/^\s+([a-zA-Z0-9_]+):\s*(.*)$/);
      if (sm) {
        const k = sm[1];
        if (updates[k] !== undefined) {
          newLines.push(`  ${k}: ${updates[k]}`);
          stateKeysWritten.add(k);
        } else {
          newLines.push(line);
        }
        continue;
      } else if (/^[a-zA-Z0-9_]+:/.test(line)) {
        inState = false;
        for (const [k, v] of Object.entries(updates)) {
          if (!stateKeysWritten.has(k)) {
            newLines.push(`  ${k}: ${v}`);
            stateKeysWritten.add(k);
          }
        }
      }
    }
    newLines.push(line);
  }

  if (inState) {
    for (const [k, v] of Object.entries(updates)) {
      if (!stateKeysWritten.has(k)) {
        newLines.push(`  ${k}: ${v}`);
        stateKeysWritten.add(k);
      }
    }
  } else if (!hasStateBlock) {
    newLines.push("state:");
    for (const [k, v] of Object.entries(updates)) {
      newLines.push(`  ${k}: ${v}`);
    }
  }

  const nowIso = new Date().toISOString();
  const finalLines = newLines.map((l) =>
    l.trim().startsWith("updated_at:") ? `  updated_at: ${nowIso}` : l
  );

  const newText = `---\n${finalLines.join("\n")}\n---\n${body}`;
  fs.writeFileSync(cardPath, newText, "utf8");
  return true;
}

function toolSnapshot(label: string = "manual"): Record<string, unknown> {
  const card = getActiveAgentCard();
  if (!fs.existsSync(card)) {
    return { status: "error", error: `Agent card ${card} not found` };
  }
  const now = new Date();
  const pad = (n: number) => n.toString().padStart(2, "0");
  const ts = `${now.getFullYear()}${pad(now.getMonth() + 1)}${pad(now.getDate())}_${pad(now.getHours())}${pad(now.getMinutes())}${pad(now.getSeconds())}`;
  const agentSlug = path.basename(card).split(".")[0];
  const snapFile = path.join(getSnapshotsDir(), `${ts}_${label}_${agentSlug}.agent.md`);
  fs.writeFileSync(snapFile, fs.readFileSync(card, "utf8"), "utf8");
  return { status: "ok", snapshot: snapFile, agent: agentSlug, label };
}

function toolPause(reason: string = "user_pause"): Record<string, unknown> {
  const card = getActiveAgentCard();
  const snapRes = toolSnapshot(`pause_${reason.slice(0, 15).replace(/\s+/g, "_")}`);
  updateAgentCardState(card, {
    mode: "rest",
    next: `paused: ${reason}`,
  });
  return {
    status: "ok",
    action: "pause",
    reason,
    mode: "rest",
    agent: path.basename(card),
    snapshot: snapRes.snapshot,
  };
}

function toolRestore(label: string = ""): Record<string, unknown> {
  const dir = getSnapshotsDir();
  const files = fs.readdirSync(dir).filter((f) => f.endsWith(".agent.md")).sort();
  if (files.length === 0) {
    return { status: "error", error: "no snapshots found in memory/snapshots" };
  }
  let target = files[files.length - 1];
  if (label) {
    for (let i = files.length - 1; i >= 0; i--) {
      if (files[i].includes(label)) {
        target = files[i];
        break;
      }
    }
  }
  const targetFull = path.join(dir, target);
  const card = getActiveAgentCard();
  fs.writeFileSync(card, fs.readFileSync(targetFull, "utf8"), "utf8");
  return { status: "ok", action: "restore", restored_from: targetFull, agent: path.basename(card) };
}

function senseState(): Record<string, unknown> {
  const card = getActiveAgentCard();
  const { data } = parseAgentFrontmatter(card);
  return {
    status: "ok",
    agent: path.basename(card),
    state: data.state || {},
    organs: data.organs || [],
  };
}

function main(): void {
  const args = parseArgs();

  // Autonomous vegetative stimuli for Stem
  if (args.stimulus) {
    const stim = args.stimulus.toLowerCase();
    let res: Record<string, unknown>;
    if (stim === "stale_goal") {
      const root = findWorkspaceRoot();
      const possibleGoals = [
        path.join(root, "GOALS.md"),
        path.join(root, "workspace", "GOALS.md"),
        path.join(root, "memory", "goals.md"),
      ];
      let goalPath = "";
      for (const gp of possibleGoals) {
        if (fs.existsSync(gp)) {
          goalPath = gp;
          break;
        }
      }
      if (!goalPath) {
        res = {
          status: "ok",
          stimulus: "stale_goal",
          triggered: false,
          message: "No active GOALS.md found",
        };
      } else {
        const stat = fs.statSync(goalPath);
        const ageHours = (Date.now() - stat.mtimeMs) / (1000 * 3600);
        const thresholdHours = parseFloat(args.threshold_hours || "12.0");
        const content = fs.readFileSync(goalPath, "utf8");
        const hasUnfinished = content.includes("- [ ]");
        const triggered = ageHours >= thresholdHours && hasUnfinished;
        res = {
          status: "ok",
          stimulus: "stale_goal",
          goal_path: goalPath,
          age_hours: Math.round(ageHours * 10) / 10,
          threshold_hours: thresholdHours,
          has_unfinished_goals: hasUnfinished,
          triggered,
        };
      }
    } else {
      res = { status: "error", error: `Unknown stimulus: ${stim}` };
    }
    console.log(JSON.stringify(res, null, 2));
    process.exit(res.status === "ok" ? 0 : 1);
  }

  const op = (args.tool || args.action || args.sense || "").toLowerCase();

  let res: Record<string, unknown>;
  if (op.includes("snapshot") || op === "save") {
    res = toolSnapshot(args.label || "manual");
  } else if (op.includes("pause")) {
    res = toolPause(args.reason || "pause");
  } else if (op.includes("restore")) {
    res = toolRestore(args.label || "");
  } else if (op.includes("sense") || op.includes("state")) {
    res = senseState();
  } else {
    res = { status: "error", error: `Unknown state action: ${op}` };
  }

  console.log(JSON.stringify(res, null, 2));
  process.exit(res.status === "ok" ? 0 : 1);
}

main();