#!/usr/bin/env node
import * as fs from "node:fs";
import * as path from "node:path";
import * as process from "node:process";

interface PonderEntry {
  ts: number;
  type: "ponder";
  thought: string;
  confidence: number;
}

interface ReflectEntry {
  ts: number;
  type: "reflect";
  synthesis: string;
}

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

function getMonologueFile(): string {
  const root = findWorkspaceRoot();
  const d = path.join(root, "memory");
  fs.mkdirSync(d, { recursive: true });
  return path.join(d, "monologue.jsonl");
}

function toolPonder(thought: string, confidence: number = 1.0): Record<string, unknown> {
  if (!thought || !thought.trim()) {
    return { status: "error", error: "thought cannot be empty" };
  }
  const mf = getMonologueFile();
  const entry: PonderEntry = {
    ts: Date.now() / 1000,
    type: "ponder",
    thought: thought.trim(),
    confidence,
  };
  fs.appendFileSync(mf, JSON.stringify(entry) + "\n", "utf8");
  return { status: "ok", recorded: "silent_thought", confidence };
}

function toolReflect(synthesis: string): Record<string, unknown> {
  if (!synthesis || !synthesis.trim()) {
    return { status: "error", error: "synthesis cannot be empty" };
  }
  const mf = getMonologueFile();
  const entry: ReflectEntry = {
    ts: Date.now() / 1000,
    type: "reflect",
    synthesis: synthesis.trim(),
  };
  fs.appendFileSync(mf, JSON.stringify(entry) + "\n", "utf8");
  return { status: "ok", recorded: "reflection" };
}

function senseStream(limit: number = 5): Record<string, unknown> {
  const mf = getMonologueFile();
  if (!fs.existsSync(mf)) {
    return { status: "ok", stream: [] };
  }
  try {
    const raw = fs.readFileSync(mf, "utf8");
    const lines = raw.split(/\r?\n/).filter((l) => l.trim().length > 0);
    const entries: unknown[] = [];
    for (const line of lines.slice(-limit)) {
      try {
        entries.push(JSON.parse(line));
      } catch {
        // ignore malformed
      }
    }
    return { status: "ok", stream: entries };
  } catch (err: unknown) {
    return { status: "error", error: String(err) };
  }
}

function main(): void {
  const args = parseArgs();
  const op = (args.tool || args.sense || "").toLowerCase();

  let res: Record<string, unknown>;
  if (op.includes("reflect") || args.synthesis) {
    res = toolReflect(args.synthesis || args.thought || "");
  } else if (op.includes("sense") || args.sense) {
    res = senseStream();
  } else {
    const conf = args.confidence ? parseFloat(args.confidence) : 1.0;
    res = toolPonder(args.thought || "", isNaN(conf) ? 1.0 : conf);
  }

  console.log(JSON.stringify(res, null, 2));
  process.exit(res.status === "ok" ? 0 : 1);
}

main();