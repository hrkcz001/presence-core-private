#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import * as process from "node:process";

interface GitRunResult {
  code: number;
  stdout: string;
  stderr: string;
}

interface GitStatusResult {
  status: "ok" | "error";
  branch?: string;
  clean?: boolean;
  staged?: Array<{ status: string; file: string }>;
  unstaged?: Array<{ status: string; file: string }>;
  untracked?: string[];
  error?: string;
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

function runGit(gitArgs: string[], maxLines: number = 40): GitRunResult {
  const env = { ...process.env, GIT_TERMINAL_PROMPT: "0", PAGER: "cat" };
  const res = spawnSync("git", ["--no-pager", ...gitArgs], {
    encoding: "utf8",
    env,
    timeout: 25000,
    windowsHide: true,
  });

  if (res.error) {
    return { code: -1, stdout: "", stderr: res.error.message };
  }

  const rawOut = res.stdout ? res.stdout.trim() : "";
  const rawErr = res.stderr ? res.stderr.trim() : "";
  const lines = rawOut ? rawOut.split(/\r?\n/) : [];

  let stdout = rawOut;
  if (lines.length > maxLines) {
    stdout = lines.slice(0, maxLines).join("\n") + `\n... [truncated ${lines.length - maxLines} lines]`;
  }

  return {
    code: res.status ?? (res.error ? -1 : 0),
    stdout,
    stderr: rawErr,
  };
}

function actionStatus(): GitStatusResult {
  const { code, stdout, stderr } = runGit(["status", "--porcelain", "-b"]);
  if (code !== 0) {
    return { status: "error", error: stderr || stdout };
  }

  const lines = stdout.split(/\r?\n/).filter((l) => l.trim().length > 0);
  const branch = lines.length > 0 ? lines[0].replace("##", "").trim() : "unknown";
  const staged: Array<{ status: string; file: string }> = [];
  const unstaged: Array<{ status: string; file: string }> = [];
  const untracked: string[] = [];

  for (const line of lines.slice(1)) {
    if (line.length < 3) continue;
    const indexStatus = line[0];
    const workStatus = line[1];
    const filePath = line.slice(3).trim();

    if (indexStatus === "?" && workStatus === "?") {
      untracked.push(filePath);
    } else {
      if (indexStatus !== " " && indexStatus !== "?") {
        staged.push({ status: indexStatus, file: filePath });
      }
      if (workStatus !== " " && workStatus !== "?") {
        unstaged.push({ status: workStatus, file: filePath });
      }
    }
  }

  return {
    status: "ok",
    branch,
    clean: staged.length === 0 && unstaged.length === 0 && untracked.length === 0,
    staged,
    unstaged,
    untracked,
  };
}

function actionDiff(filePath?: string, maxLines: number = 40): Record<string, unknown> {
  const gitArgs = ["diff"];
  if (filePath) {
    gitArgs.push("--", filePath);
  }
  const { code, stdout, stderr } = runGit(gitArgs, maxLines);
  if (code !== 0) {
    return { status: "error", error: stderr || stdout };
  }
  return { status: "ok", diff: stdout };
}

function actionLog(maxCount: number = 10): Record<string, unknown> {
  const { code, stdout, stderr } = runGit(["log", `-n${maxCount}`, "--oneline"]);
  if (code !== 0) {
    return { status: "error", error: stderr || stdout };
  }
  const commits: Array<{ hash: string; message: string }> = [];
  for (const line of stdout.split(/\r?\n/).filter((l) => l.trim().length > 0)) {
    const idx = line.indexOf(" ");
    if (idx !== -1) {
      commits.push({ hash: line.slice(0, idx), message: line.slice(idx + 1) });
    } else {
      commits.push({ hash: line, message: "" });
    }
  }
  return { status: "ok", commits };
}

function actionCheckpoint(message: string, filePath?: string): Record<string, unknown> {
  if (!message || !message.trim()) {
    return { status: "error", error: "Commit message is required for checkpoint" };
  }

  // 1. Stage
  const addArgs = ["add", filePath ? filePath : "-A"];
  const addRes = runGit(addArgs);
  if (addRes.code !== 0) {
    return { status: "error", error: `git add failed: ${addRes.stderr || addRes.stdout}` };
  }

  // 2. Commit
  const commitRes = runGit(["commit", "-m", message.trim()]);
  if (commitRes.code !== 0) {
    if (commitRes.stdout.includes("nothing to commit") || commitRes.stderr.includes("nothing to commit")) {
      return { status: "noop", message: "Nothing to commit, working tree clean" };
    }
    return { status: "error", error: `git commit failed: ${commitRes.stderr || commitRes.stdout}` };
  }

  // 3. Head rev
  const revRes = runGit(["rev-parse", "--short", "HEAD"]);
  const commitHash = revRes.code === 0 ? revRes.stdout.trim() : "unknown";

  return {
    status: "ok",
    commit: commitHash,
    message: message.trim(),
    detail: commitRes.stdout,
  };
}

function main(): void {
  const args = parseArgs();
  const action = (args.action || args.tool || "status").toLowerCase();
  const maxLines = args.max_lines ? parseInt(args.max_lines, 10) : 40;

  let res: Record<string, unknown>;
  if (action === "status") {
    res = actionStatus() as unknown as Record<string, unknown>;
  } else if (action === "diff") {
    res = actionDiff(args.path, maxLines);
  } else if (action === "log") {
    const maxCount = args.max_count ? parseInt(args.max_count, 10) : 10;
    res = actionLog(maxCount);
  } else if (action === "checkpoint") {
    res = actionCheckpoint(args.message || "", args.path);
  } else {
    res = { status: "error", error: `Unknown action: ${action}` };
  }

  console.log(JSON.stringify(res, null, 2));
  process.exit(res.status === "ok" || res.status === "noop" ? 0 : 1);
}

main();