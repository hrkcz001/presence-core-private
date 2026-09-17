#!/usr/bin/env python3
import sys
import os
import argparse
import json
import time
from pathlib import Path

def find_workspace_root() -> Path:
    env_root = os.environ.get("PRESENCE_WORKSPACE")
    if env_root and Path(env_root).is_dir():
        return Path(env_root)
    p = Path(__file__).resolve().parent
    while p.parent != p:
        if (p / "memory").is_dir() or (p / "AGENTS.md").is_file():
            return p
        p = p.parent
    return Path.cwd()

def get_outbox_file() -> Path:
    d = find_workspace_root() / "memory"
    d.mkdir(parents=True, exist_ok=True)
    return d / "outbox.jsonl"

def get_inbox_file() -> Path:
    d = find_workspace_root() / "memory"
    d.mkdir(parents=True, exist_ok=True)
    return d / "inbox.jsonl"

def tool_send_reply(message: str, channel: str = "active") -> dict:
    if not message or not message.strip():
        return {"status": "error", "error": "message cannot be empty"}
    
    outbox = get_outbox_file()
    entry = {
        "ts": time.time(),
        "channel": channel,
        "type": "reply",
        "message": message.strip()
    }
    with open(outbox, "a", encoding="utf-8") as f:
        f.write(json.dumps(entry, ensure_ascii=False) + "\n")

    return {
        "status": "ok",
        "action": "sent",
        "channel": channel,
        "delivered_chars": len(message.strip()),
        "preview": message.strip()[:80]
    }

def tool_send_status(status: str) -> dict:
    if not status or not status.strip():
        return {"status": "error", "error": "status cannot be empty"}
    outbox = get_outbox_file()
    entry = {
        "ts": time.time(),
        "channel": "active",
        "type": "status",
        "status": status.strip()
    }
    with open(outbox, "a", encoding="utf-8") as f:
        f.write(json.dumps(entry, ensure_ascii=False) + "\n")
    return {"status": "ok", "action": "status_reported"}

def sense_inbox() -> dict:
    inbox = get_inbox_file()
    if not inbox.is_file():
        return {"status": "ok", "inbox": []}
    lines = inbox.read_text(encoding="utf-8", errors="replace").splitlines()
    entries = []
    for line in lines[-5:]:
        try:
            entries.append(json.loads(line))
        except Exception:
            continue
    return {"status": "ok", "inbox": entries}

def main():
    parser = argparse.ArgumentParser(description="Organ Channel (External Client Communication)")
    parser.add_argument("--tool", default="", help="Tool name")
    parser.add_argument("--message", default="", help="Message text")
    parser.add_argument("--channel", default="active", help="Channel id")
    parser.add_argument("--status", default="", help="Status update")
    parser.add_argument("--sense", default="", help="Sense query")

    args, _ = parser.parse_known_args()
    op = (args.tool or args.sense or "").lower()

    if "status" in op or args.status:
        res = tool_send_status(args.status or args.message)
    elif "inbox" in op or args.sense:
        res = sense_inbox()
    else:
        res = tool_send_reply(args.message, args.channel)

    print(json.dumps(res, ensure_ascii=False, indent=2))
    sys.exit(0 if res.get("status") == "ok" else 1)

if __name__ == "__main__":
    main()
