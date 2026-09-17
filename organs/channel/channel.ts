#!/usr/bin/env bun
import * as fs from "node:fs";
import * as path from "node:path";
import * as process from "node:process";

interface ReplyEntry {
  ts: number;
  channel: string;
  type: string;
  message: string;
}

interface StatusEntry {
  ts: number;
  channel: string;
  type: string;
  status: string;
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

function getOutboxFile(): string {
  const mem = path.join(findWorkspaceRoot(), "memory");
  fs.mkdirSync(mem, { recursive: true });
  return path.join(mem, "outbox.jsonl");
}

function getInboxFile(): string {
  const mem = path.join(findWorkspaceRoot(), "memory");
  fs.mkdirSync(mem, { recursive: true });
  return path.join(mem, "inbox.jsonl");
}

function toolSendReply(message: string, channel: string = "active"): Record<string, unknown> {
  if (!message || !message.trim()) {
    return { status: "error", error: "message cannot be empty" };
  }
  const outbox = getOutboxFile();
  const entry: ReplyEntry = {
    ts: Date.now() / 1000,
    channel,
    type: "reply",
    message: message.trim(),
  };
  fs.appendFileSync(outbox, JSON.stringify(entry) + "\n", "utf8");
  return {
    status: "ok",
    action: "sent",
    channel,
    delivered_chars: message.trim().length,
    preview: message.trim().slice(0, 80),
  };
}

function toolSendStatus(status: string): Record<string, unknown> {
  if (!status || !status.trim()) {
    return { status: "error", error: "status cannot be empty" };
  }
  const outbox = getOutboxFile();
  const entry: StatusEntry = {
    ts: Date.now() / 1000,
    channel: "active",
    type: "status",
    status: status.trim(),
  };
  fs.appendFileSync(outbox, JSON.stringify(entry) + "\n", "utf8");
  return { status: "ok", action: "status_reported" };
}

function senseInbox(): Record<string, unknown> {
  const inbox = getInboxFile();
  if (!fs.existsSync(inbox)) {
    return { status: "ok", inbox: [] };
  }
  try {
    const raw = fs.readFileSync(inbox, "utf8");
    const lines = raw.split(/\r?\n/).filter((l) => l.trim().length > 0);
    const entries: unknown[] = [];
    for (const line of lines.slice(-5)) {
      try {
        entries.push(JSON.parse(line));
      } catch {
        // ignore malformed line
      }
    }
    return { status: "ok", inbox: entries };
  } catch (err: unknown) {
    return { status: "error", error: String(err) };
  }
}

function main(): void {
  const args = parseArgs();
  const op = (args.tool || args.sense || "").toLowerCase();

  let res: Record<string, unknown>;
  if (op.includes("status") || args.status) {
    res = toolSendStatus(args.status || args.message || "");
  } else if (op.includes("inbox") || args.sense) {
    res = senseInbox();
  } else {
    res = toolSendReply(args.message || "", args.channel || "active");
  }

  console.log(JSON.stringify(res, null, 2));
  process.exit(res.status === "ok" ? 0 : 1);
}

main();