#!/usr/bin/env python3
import sys
import os
import argparse
import json
import datetime
import re
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

def get_active_agent_card() -> Path:
    root = find_workspace_root()
    agent_name = os.environ.get("PRESENCE_AGENT", "presence")
    card = root / "agents" / f"{agent_name}.agent.md"
    if card.is_file():
        return card
    fallback = root / "agents" / "presence.agent.md"
    if fallback.is_file():
        return fallback
    return card

def get_journal_file() -> Path:
    root = find_workspace_root()
    return root / "memory" / "journal.md"

def parse_agent_card_intent() -> str:
    card = get_active_agent_card()
    if not card.is_file():
        return "none"
    text = card.read_text(encoding="utf-8", errors="replace").lstrip("\ufeff")
    in_state = False
    for line in text.splitlines():
        if line.strip() == "state:" or line.startswith("state:"):
            in_state = True
            continue
        if in_state:
            sm = re.match(r"^\s+next:\s*(.*)$", line)
            if sm:
                return sm.group(1).strip()
            elif re.match(r"^[a-zA-Z0-9_]+:", line):
                break
    return "none"

def update_agent_card_next(step: str):
    card = get_active_agent_card()
    if not card.is_file():
        return False
    text = card.read_text(encoding="utf-8", errors="replace").lstrip("\ufeff")
    m = re.match(r"^---\r?\n(.*?)\r?\n---\r?\n(.*)$", text, re.DOTALL)
    if not m:
        return False
    fm_raw = m.group(1)
    body = m.group(2)
    lines = fm_raw.splitlines()
    new_lines = []
    in_state = False
    replaced = False

    for line in lines:
        if line.strip() == "state:" or line.startswith("state:"):
            in_state = True
            new_lines.append(line)
            continue
        if in_state:
            sm = re.match(r"^\s+next:\s*(.*)$", line)
            if sm:
                new_lines.append(f"  next: {step}")
                replaced = True
                continue
            elif re.match(r"^[a-zA-Z0-9_]+:", line):
                in_state = False
                if not replaced:
                    new_lines.append(f"  next: {step}")
                    replaced = True
        new_lines.append(line)

    if in_state and not replaced:
        new_lines.append(f"  next: {step}")

    new_text = "---\n" + "\n".join(new_lines) + "\n---\n" + body
    card.write_text(new_text, encoding="utf-8")
    return True

def tool_plan_step(action: str, step: str = "") -> dict:
    card = get_active_agent_card()
    current_next = parse_agent_card_intent()

    if action in ("show", "status"):
        return {
            "status": "ok",
            "current_intent": current_next,
            "agent_card": str(card)
        }
    elif action == "set":
        if not step:
            return {"status": "error", "error": "step is required for set"}
        update_agent_card_next(step)
        return {"status": "ok", "action": "set", "new_intent": step, "agent": card.name}
    elif action == "complete":
        update_agent_card_next("none")
        return {"status": "ok", "action": "complete", "completed_step": step or current_next, "agent": card.name}
    else:
        return {"status": "error", "error": f"Unknown plan action: {action}"}

def tool_journal_append(entry: str) -> dict:
    if not entry or not entry.strip():
        return {"status": "error", "error": "entry cannot be empty"}
    jf = get_journal_file()
    ts = datetime.datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    formatted = f"\n### [{ts}]\n{entry.strip()}\n"
    with open(jf, "a", encoding="utf-8") as f:
        f.write(formatted)
    return {"status": "ok", "action": "journal_appended", "bytes": len(formatted)}

def sense_standing_intent() -> dict:
    intent = parse_agent_card_intent()
    card = get_active_agent_card()
    return {
        "status": "ok",
        "standing_intent": intent,
        "agent": card.name
    }

def main():
    parser = argparse.ArgumentParser(description="Organ Plan & Journal (Bound to Agent Card)")
    parser.add_argument("--tool", default="", help="Tool name")
    parser.add_argument("--action", default="", help="Plan action")
    parser.add_argument("--step", default="", help="Step text")
    parser.add_argument("--entry", default="", help="Journal entry text")
    parser.add_argument("--sense", default="", help="Sense query")

    args, _ = parser.parse_known_args()
    op = (args.tool or args.action or args.sense or "").lower()

    if "journal" in op or args.entry:
        res = tool_journal_append(args.entry or args.step)
    elif "intent" in op or "sense" in op:
        res = sense_standing_intent()
    else:
        act = args.action or ("set" if args.step else "show")
        res = tool_plan_step(act, args.step)

    print(json.dumps(res, ensure_ascii=False, indent=2))
    sys.exit(0 if res.get("status") == "ok" else 1)

if __name__ == "__main__":
    main()
