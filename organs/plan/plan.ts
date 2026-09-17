#!/usr/bin/env bun
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

function getJournalFile(): string {
  const root = findWorkspaceRoot();
  const mem = path.join(root, "memory");
  fs.mkdirSync(mem, { recursive: true });
  return path.join(mem, "journal.md");
}

function parseAgentCardIntent(): string {
  const card = getActiveAgentCard();
  if (!fs.existsSync(card)) {
    return "none";
  }
  const text = fs.readFileSync(card, "utf8").replace(/^\uFEFF/, "");
  let inState = false;
  for (const line of text.split(/\r?\n/)) {
    if (line.trim() === "state:" || line.startsWith("state:")) {
      inState = true;
      continue;
    }
    if (inState) {
      const sm = line.match(/^\s+next:\s*(.*)$/);
      if (sm) {
        return sm[1].trim();
      } else if (/^[a-zA-Z0-9_]+:/.test(line)) {
        break;
      }
    }
  }
  return "none";
}

function updateAgentCardNext(step: string): boolean {
  const card = getActiveAgentCard();
  if (!fs.existsSync(card)) {
    return false;
  }
  const text = fs.readFileSync(card, "utf8").replace(/^\uFEFF/, "");
  const m = text.match(/^---\r?\n([\s\S]*?)\r?\n---\r?\n([\s\S]*)$/);
  if (!m) {
    return false;
  }
  const fmRaw = m[1];
  const body = m[2];
  const lines = fmRaw.split(/\r?\n/);
  const newLines: string[] = [];
  let inState = false;
  let replaced = false;

  for (const line of lines) {
    if (line.trim() === "state:" || line.startsWith("state:")) {
      inState = true;
      newLines.append ? null : null;
      newLines.push(line);
      continue;
    }
    if (inState) {
      const sm = line.match(/^\s+next:\s*(.*)$/);
      if (sm) {
        newLines.push(`  next: ${step}`);
        replaced = true;
        continue;
      } else if (/^[a-zA-Z0-9_]+:/.test(line)) {
        inState = false;
        if (!replaced) {
          newLines.push(`  next: ${step}`);
          replaced = true;
        }
      }
    }
    newLines.push(line);
  }

  if (inState && !replaced) {
    newLines.push(`  next: ${step}`);
  }

  const newText = `---\n${newLines.join("\n")}\n---\n${body}`;
  fs.writeFileSync(card, newText, "utf8");
  return true;
}

function toolPlanStep(action: string, step: string = ""): Record<string, unknown> {
  const card = getActiveAgentCard();
  const currentNext = parseAgentCardIntent();

  if (action === "show" || action === "status") {
    return {
      status: "ok",
      current_intent: currentNext,
      agent_card: card,
    };
  } else if (action === "set") {
    if (!step) {
      return { status: "error", error: "step is required for set" };
    }
    updateAgentCardNext(step);
    return { status: "ok", action: "set", new_intent: step, agent: path.basename(card) };
  } else if (action === "complete") {
    updateAgentCardNext("none");
    return {
      status: "ok",
      action: "complete",
      completed_step: step || currentNext,
      agent: path.basename(card),
    };
  } else {
    return { status: "error", error: `Unknown plan action: ${action}` };
  }
}

function toolJournalAppend(entry: string): Record<string, unknown> {
  if (!entry || !entry.trim()) {
    return { status: "error", error: "entry cannot be empty" };
  }
  const jf = getJournalFile();
  const now = new Date();
  const pad = (n: number) => n.toString().padStart(2, "0");
  const ts = `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())} ${pad(now.getHours())}:${pad(now.getMinutes())}:${pad(now.getSeconds())}`;
  const formatted = `\n### [${ts}]\n${entry.trim()}\n`;
  fs.appendFileSync(jf, formatted, "utf8");
  return { status: "ok", action: "journal_appended", bytes: Buffer.byteLength(formatted, "utf8") };
}

function senseStandingIntent(): Record<string, unknown> {
  const intent = parseAgentCardIntent();
  const card = getActiveAgentCard();
  return {
    status: "ok",
    standing_intent: intent,
    agent: path.basename(card),
  };
}

function main(): void {
  const args = parseArgs();
  const op = (args.tool || args.action || args.sense || "").toLowerCase();

  let res: Record<string, unknown>;
  if (op.includes("journal") || args.entry) {
    res = toolJournalAppend(args.entry || args.step || "");
  } else if (op.includes("intent") || op.includes("sense")) {
    res = senseStandingIntent();
  } else {
    const act = args.action || (args.step ? "set" : "show");
    res = toolPlanStep(act, args.step || "");
  }

  console.log(JSON.stringify(res, null, 2));
  process.exit(res.status === "ok" ? 0 : 1);
}

main();