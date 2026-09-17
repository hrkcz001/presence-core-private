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

def get_snapshots_dir() -> Path:
    d = find_workspace_root() / "memory" / "snapshots"
    d.mkdir(parents=True, exist_ok=True)
    return d

def parse_agent_frontmatter(card_path: Path):
    if not card_path.is_file():
        return {}, "", ""
    text = card_path.read_text(encoding="utf-8", errors="replace").lstrip("\ufeff")
    m = re.match(r"^---\r?\n(.*?)\r?\n---\r?\n(.*)$", text, re.DOTALL)
    if not m:
        return {}, "", text
    fm_raw = m.group(1)
    body = m.group(2)
    data = {}
    in_state = False
    state_dict = {}
    organs_list = []
    in_organs = False

    for line in fm_raw.splitlines():
        if line.strip() == "state:" or line.startswith("state:"):
            in_state = True
            in_organs = False
            continue
        if line.strip() == "organs:" or line.startswith("organs:"):
            in_organs = True
            in_state = False
            continue
        if in_state:
            sm = re.match(r"^\s+([a-zA-Z0-9_]+):\s*(.*)$", line)
            if sm:
                state_dict[sm.group(1)] = sm.group(2).strip()
                continue
            elif re.match(r"^[a-zA-Z0-9_]+:", line):
                in_state = False
        if in_organs:
            om = re.match(r"^\s*-\s*([a-zA-Z0-9_-]+)", line)
            if om:
                organs_list.append(om.group(1))
                continue
            elif re.match(r"^[a-zA-Z0-9_]+:", line):
                in_organs = False

        m_top = re.match(r"^([a-zA-Z0-9_]+):\s*(.*)$", line)
        if m_top:
            data[m_top.group(1)] = m_top.group(2).strip()

    data["state"] = state_dict
    data["organs"] = organs_list
    return data, fm_raw, body

def update_agent_card_state(card_path: Path, updates: dict):
    if not card_path.is_file():
        return False
    text = card_path.read_text(encoding="utf-8", errors="replace").lstrip("\ufeff")
    m = re.match(r"^---\r?\n(.*?)\r?\n---\r?\n(.*)$", text, re.DOTALL)
    if not m:
        return False
    fm_raw = m.group(1)
    body = m.group(2)
    
    lines = fm_raw.splitlines()
    new_lines = []
    in_state = False
    state_keys_written = set()
    has_state_block = False

    for line in lines:
        if line.strip() == "state:" or line.startswith("state:"):
            in_state = True
            has_state_block = True
            new_lines.append("state:")
            continue
        if in_state:
            sm = re.match(r"^\s+([a-zA-Z0-9_]+):\s*(.*)$", line)
            if sm:
                k = sm.group(1)
                if k in updates:
                    new_lines.append(f"  {k}: {updates[k]}")
                    state_keys_written.add(k)
                else:
                    new_lines.append(line)
                continue
            elif re.match(r"^[a-zA-Z0-9_]+:", line):
                in_state = False
                for k, v in updates.items():
                    if k not in state_keys_written:
                        new_lines.append(f"  {k}: {v}")
                        state_keys_written.add(k)
        new_lines.append(line)

    if in_state:
        for k, v in updates.items():
            if k not in state_keys_written:
                new_lines.append(f"  {k}: {v}")
                state_keys_written.add(k)
    elif not has_state_block:
        new_lines.append("state:")
        for k, v in updates.items():
            new_lines.append(f"  {k}: {v}")

    now = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    final_lines = []
    for line in new_lines:
        if line.strip().startswith("updated_at:"):
            final_lines.append(f"  updated_at: {now}")
        else:
            final_lines.append(line)

    new_text = "---\n" + "\n".join(final_lines) + "\n---\n" + body
    card_path.write_text(new_text, encoding="utf-8")
    return True

def tool_snapshot(label: str = "manual") -> dict:
    card = get_active_agent_card()
    if not card.is_file():
        return {"status": "error", "error": f"Agent card {card} not found"}
    ts = datetime.datetime.now().strftime("%Y%m%d_%H%M%S")
    agent_slug = card.stem.split(".")[0]
    snap_file = get_snapshots_dir() / f"{ts}_{label}_{agent_slug}.agent.md"
    snap_file.write_text(card.read_text(encoding="utf-8", errors="replace"), encoding="utf-8")
    return {"status": "ok", "snapshot": str(snap_file), "agent": agent_slug, "label": label}

def tool_pause(reason: str = "user_pause") -> dict:
    card = get_active_agent_card()
    snap_res = tool_snapshot(f"pause_{reason[:15].replace(' ', '_')}")
    update_agent_card_state(card, {
        "mode": "rest",
        "next": f"paused: {reason}"
    })
    return {
        "status": "ok",
        "action": "pause",
        "reason": reason,
        "mode": "rest",
        "agent": card.name,
        "snapshot": snap_res.get("snapshot")
    }

def tool_restore(label: str = "") -> dict:
    snaps = sorted(get_snapshots_dir().glob("*.agent.md"))
    if not snaps:
        return {"status": "error", "error": "no snapshots found in memory/snapshots"}
    target = None
    if label:
        for s in reversed(snaps):
            if label in s.name:
                target = s
                break
    if not target:
        target = snaps[-1]
    card = get_active_agent_card()
    card.write_text(target.read_text(encoding="utf-8", errors="replace"), encoding="utf-8")
    return {"status": "ok", "action": "restore", "restored_from": str(target), "agent": card.name}

def sense_state() -> dict:
    card = get_active_agent_card()
    data, _, _ = parse_agent_frontmatter(card)
    return {
        "status": "ok",
        "agent": card.name,
        "state": data.get("state", {}),
        "organs": data.get("organs", [])
    }

def main():
    parser = argparse.ArgumentParser(description="Organ State (Agent Card State Preservation)")
    parser.add_argument("--tool", default="", help="Tool name")
    parser.add_argument("--action", default="", help="Action name")
    parser.add_argument("--label", default="", help="Snapshot label")
    parser.add_argument("--reason", default="pause", help="Pause reason")
    parser.add_argument("--sense", default="", help="Sense name")

    args, _ = parser.parse_known_args()
    op = (args.tool or args.action or args.sense or "").lower()

    if "snapshot" in op:
        res = tool_snapshot(args.label)
    elif "pause" in op:
        res = tool_pause(args.reason)
    elif "restore" in op:
        res = tool_restore(args.label)
    elif "sense" in op or "state" in op:
        res = sense_state()
    else:
        res = {"status": "error", "error": f"Unknown state action: {op}"}

    print(json.dumps(res, ensure_ascii=False, indent=2))
    sys.exit(0 if res.get("status") == "ok" else 1)

if __name__ == "__main__":
    main()
