#!/usr/bin/env node
import * as fs from "node:fs";
import * as path from "node:path";
import * as process from "node:process";

interface FileEntry {
  name: string;
  is_dir: boolean;
  size: number | null;
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

function resolvePath(p: string): string {
  const target = path.resolve(process.cwd(), p);
  return target;
}

function toolRead(pathStr: string): Record<string, unknown> {
  if (!pathStr) {
    return { status: "error", error: "path is required" };
  }
  const p = resolvePath(pathStr);
  if (!fs.existsSync(p) || !fs.statSync(p).isFile()) {
    return { status: "error", error: `File not found: ${p}` };
  }
  try {
    const content = fs.readFileSync(p, "utf8");
    return { status: "ok", path: p, content };
  } catch (err: unknown) {
    return { status: "error", error: String(err) };
  }
}

function toolWrite(pathStr: string, content: string): Record<string, unknown> {
  if (!pathStr) {
    return { status: "error", error: "path is required" };
  }
  const p = resolvePath(pathStr);
  try {
    fs.mkdirSync(path.dirname(p), { recursive: true });
    fs.writeFileSync(p, content, "utf8");
    return { status: "ok", path: p, bytes: Buffer.byteLength(content, "utf8") };
  } catch (err: unknown) {
    return { status: "error", error: String(err) };
  }
}

function toolList(pathStr?: string): Record<string, unknown> {
  const p = pathStr ? resolvePath(pathStr) : process.cwd();
  if (!fs.existsSync(p) || !fs.statSync(p).isDirectory()) {
    return { status: "error", error: `Directory not found: ${p}` };
  }
  try {
    const rawEntries = fs.readdirSync(p, { withFileTypes: true });
    const entries: FileEntry[] = rawEntries
      .map((entry) => {
        const full = path.join(p, entry.name);
        const isDir = entry.isDirectory();
        let size: number | null = null;
        if (!isDir) {
          try {
            size = fs.statSync(full).size;
          } catch {
            // ignore
          }
        }
        return { name: entry.name, is_dir: isDir, size };
      })
      .sort((a, b) => {
        if (a.is_dir !== b.is_dir) {
          return a.is_dir ? -1 : 1;
        }
        return a.name.toLowerCase().localeCompare(b.name.toLowerCase());
      });
    return { status: "ok", path: p, entries };
  } catch (err: unknown) {
    return { status: "error", error: String(err) };
  }
}

function senseFsChanges(sinceSeconds: number = 60): Record<string, unknown> {
  const now = Date.now() / 1000;
  const changed: Array<{ path: string; mtime: number }> = [];
  const cwd = process.cwd();

  function scan(dir: string): void {
    if (changed.length >= 20) return;
    let entries: fs.Dirent[];
    try {
      entries = fs.readdirSync(dir, { withFileTypes: true });
    } catch {
      return;
    }
    for (const ent of entries) {
      if (ent.name.startsWith(".") || ent.name === "target" || ent.name === "node_modules") {
        continue;
      }
      const full = path.join(dir, ent.name);
      try {
        const stat = fs.statSync(full);
        const mtime = stat.mtimeMs / 1000;
        if (now - mtime <= sinceSeconds) {
          changed.push({ path: path.relative(cwd, full), mtime });
        }
        if (ent.isDirectory()) {
          scan(full);
        }
      } catch {
        // ignore
      }
    }
  }

  scan(cwd);
  return { status: "ok", recent_changes: changed.slice(0, 20) };
}

function main(): void {
  const args = parseArgs();
  let op = (args.tool || args.action || "").toLowerCase();

  if (!op) {
    if (args.content !== undefined) {
      op = "write_file";
    } else if (args.path) {
      op = "read_file";
    } else if (args.sense) {
      op = "sense";
    }
  }

  let res: Record<string, unknown>;
  if (op === "read" || op === "read_file" || op === "cat") {
    res = toolRead(args.path);
  } else if (op === "write" || op === "write_file") {
    res = toolWrite(args.path, args.content || "");
  } else if (op === "list" || op === "list_files" || op === "ls") {
    res = toolList(args.path);
  } else if (op === "fs_changes" || op === "sense") {
    res = senseFsChanges();
  } else {
    res = { status: "error", error: `Unknown IO operation: ${op}` };
  }

  console.log(JSON.stringify(res, null, 2));
  process.exit(res.status === "ok" ? 0 : 1);
}

main();