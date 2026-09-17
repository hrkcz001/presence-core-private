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

def get_monologue_file() -> Path:
    root = find_workspace_root()
    d = root / "memory"
    d.mkdir(parents=True, exist_ok=True)
    return d / "monologue.jsonl"

def tool_ponder(thought: str, confidence: float = 1.0) -> dict:
    if not thought or not thought.strip():
        return {"status": "error", "error": "thought cannot be empty"}
    mf = get_monologue_file()
    entry = {
        "ts": time.time(),
        "type": "ponder",
        "thought": thought.strip(),
        "confidence": confidence
    }
    with open(mf, "a", encoding="utf-8") as f:
        f.write(json.dumps(entry, ensure_ascii=False) + "\n")
    return {"status": "ok", "recorded": "silent_thought", "confidence": confidence}

def tool_reflect(synthesis: str) -> dict:
    if not synthesis or not synthesis.strip():
        return {"status": "error", "error": "synthesis cannot be empty"}
    mf = get_monologue_file()
    entry = {
        "ts": time.time(),
        "type": "reflect",
        "synthesis": synthesis.strip()
    }
    with open(mf, "a", encoding="utf-8") as f:
        f.write(json.dumps(entry, ensure_ascii=False) + "\n")
    return {"status": "ok", "recorded": "reflection"}

def sense_stream(limit: int = 5) -> dict:
    mf = get_monologue_file()
    if not mf.is_file():
        return {"status": "ok", "stream": []}
    lines = mf.read_text(encoding="utf-8", errors="replace").splitlines()
    entries = []
    for line in lines[-limit:]:
        try:
            entries.append(json.loads(line))
        except Exception:
            continue
    return {"status": "ok", "stream": entries}

def main():
    parser = argparse.ArgumentParser(description="Organ Monologue")
    parser.add_argument("--tool", default="", help="Tool name")
    parser.add_argument("--thought", default="", help="Thought text")
    parser.add_argument("--synthesis", default="", help="Synthesis text")
    parser.add_argument("--confidence", type=float, default=1.0, help="Confidence")
    parser.add_argument("--sense", default="", help="Sense query")

    args, _ = parser.parse_known_args()
    op = (args.tool or args.sense or "").lower()

    if "reflect" in op or args.synthesis:
        res = tool_reflect(args.synthesis or args.thought)
    elif "sense" in op or args.sense:
        res = sense_stream()
    else:
        res = tool_ponder(args.thought, args.confidence)

    print(json.dumps(res, ensure_ascii=False, indent=2))
    sys.exit(0 if res.get("status") == "ok" else 1)

if __name__ == "__main__":
    main()
