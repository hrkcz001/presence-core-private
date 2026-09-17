#!/usr/bin/env python3
import sys
import os
import argparse
import subprocess
import json

def run_git(args: list[str], max_lines: int = 40) -> tuple[int, str, str]:
    env = os.environ.copy()
    env["GIT_TERMINAL_PROMPT"] = "0"
    env["PAGER"] = "cat"
    base_cmd = ["git", "--no-pager"] + args
    try:
        proc = subprocess.run(
            base_cmd,
            capture_output=True,
            text=True,
            timeout=25,
            env=env,
            encoding="utf-8",
            errors="replace"
        )
        stdout_lines = proc.stdout.splitlines()
        if len(stdout_lines) > max_lines:
            stdout = "\n".join(stdout_lines[:max_lines]) + f"\n... [truncated {len(stdout_lines) - max_lines} lines]"
        else:
            stdout = proc.stdout.strip()
        stderr = proc.stderr.strip()
        return proc.returncode, stdout, stderr
    except subprocess.TimeoutExpired:
        return -1, "", "Git command timed out after 25s"
    except Exception as e:
        return -1, "", f"Git execution error: {e}"

def action_status() -> dict:
    code, out, err = run_git(["status", "--porcelain", "-b"])
    if code != 0:
        return {"status": "error", "error": err or out}

    lines = out.splitlines()
    branch = lines[0].replace("##", "").strip() if lines else "unknown"
    staged = []
    unstaged = []
    untracked = []

    for line in lines[1:]:
        if len(line) < 3:
            continue
        index_status = line[0]
        work_status = line[1]
        file_path = line[3:].strip()
        if index_status == '?' and work_status == '?':
            untracked.append(file_path)
        else:
            if index_status not in (' ', '?'):
                staged.append({"status": index_status, "file": file_path})
            if work_status not in (' ', '?'):
                unstaged.append({"status": work_status, "file": file_path})

    return {
        "status": "ok",
        "branch": branch,
        "clean": len(staged) == 0 and len(unstaged) == 0 and len(untracked) == 0,
        "staged": staged,
        "unstaged": unstaged,
        "untracked": untracked
    }

def action_diff(path: str = None, max_lines: int = 40) -> dict:
    git_args = ["diff"]
    if path:
        git_args.extend(["--", path])
    code, out, err = run_git(git_args, max_lines=max_lines)
    if code != 0:
        return {"status": "error", "error": err or out}
    return {
        "status": "ok",
        "diff": out
    }

def action_log(max_lines: int = 10) -> dict:
    code, out, err = run_git(["log", f"-n{max_lines}", "--oneline"])
    if code != 0:
        return {"status": "error", "error": err or out}
    commits = []
    for line in out.splitlines():
        parts = line.strip().split(" ", 1)
        if len(parts) == 2:
            commits.append({"hash": parts[0], "message": parts[1]})
        elif parts:
            commits.append({"hash": parts[0], "message": ""})
    return {
        "status": "ok",
        "commits": commits
    }

def action_checkpoint(message: str, path: str = None) -> dict:
    if not message or not message.strip():
        return {"status": "error", "error": "Commit message is required for checkpoint"}

    # Stage
    add_args = ["add", path if path else "-A"]
    code, out, err = run_git(add_args)
    if code != 0:
        return {"status": "error", "error": f"git add failed: {err or out}"}

    # Commit
    code, out, err = run_git(["commit", "-m", message.strip()])
    if code != 0:
        if "nothing to commit" in out or "nothing to commit" in err:
            return {"status": "noop", "message": "Nothing to commit, working tree clean"}
        return {"status": "error", "error": f"git commit failed: {err or out}"}

    # Get last commit
    c_code, c_out, _ = run_git(["rev-parse", "--short", "HEAD"])
    commit_hash = c_out.strip() if c_code == 0 else "unknown"

    return {
        "status": "ok",
        "commit": commit_hash,
        "message": message.strip(),
        "detail": out
    }

def main():
    parser = argparse.ArgumentParser(description="Structured Git operations tool")
    parser.add_argument("--action", required=True, choices=["status", "diff", "log", "checkpoint"])
    parser.add_argument("--message", default="", help="Commit message for checkpoint")
    parser.add_argument("--path", default=None, help="Target file or folder")
    parser.add_argument("--max_lines", type=int, default=40, help="Max output lines")

    args = parser.parse_args()

    if args.action == "status":
        res = action_status()
    elif args.action == "diff":
        res = action_diff(path=args.path, max_lines=args.max_lines)
    elif args.action == "log":
        res = action_log(max_lines=args.max_lines)
    elif args.action == "checkpoint":
        res = action_checkpoint(message=args.message, path=args.path)
    else:
        res = {"status": "error", "error": f"Unknown action: {args.action}"}

    print(json.dumps(res, ensure_ascii=False, indent=2))
    sys.exit(0 if res.get("status") in ("ok", "noop") else 1)

if __name__ == "__main__":
    main()
