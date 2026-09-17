#!/usr/bin/env python3
import sys
import os
import argparse
import json
from pathlib import Path

def resolve_path(p: str) -> Path:
    target = Path(p)
    if not target.is_absolute():
        target = Path.cwd() / target
    return target.resolve()

def tool_read(path_str: str) -> dict:
    if not path_str:
        return {"status": "error", "error": "path is required"}
    p = resolve_path(path_str)
    if not p.is_file():
        return {"status": "error", "error": f"File not found: {p}"}
    try:
        content = p.read_text(encoding="utf-8", errors="replace")
        return {"status": "ok", "path": str(p), "content": content}
    except Exception as e:
        return {"status": "error", "error": str(e)}

def tool_write(path_str: str, content: str) -> dict:
    if not path_str:
        return {"status": "error", "error": "path is required"}
    p = resolve_path(path_str)
    try:
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(content, encoding="utf-8")
        return {"status": "ok", "path": str(p), "bytes": len(content.encode("utf-8"))}
    except Exception as e:
        return {"status": "error", "error": str(e)}

def tool_list(path_str: str = None) -> dict:
    p = resolve_path(path_str) if path_str else Path.cwd()
    if not p.is_dir():
        return {"status": "error", "error": f"Directory not found: {p}"}
    try:
        entries = []
        for item in sorted(p.iterdir(), key=lambda x: (not x.is_dir(), x.name.lower())):
            entries.append({
                "name": item.name,
                "is_dir": item.is_dir(),
                "size": item.stat().st_size if item.is_file() else None
            })
        return {"status": "ok", "path": str(p), "entries": entries}
    except Exception as e:
        return {"status": "error", "error": str(e)}

def sense_fs_changes(since_seconds: int = 60) -> dict:
    import time
    now = time.time()
    changed = []
    for p in Path.cwd().rglob("*"):
        if any(part.startswith(".") or part == "target" or part == "node_modules" for part in p.parts):
            continue
        try:
            mtime = p.stat().st_mtime
            if now - mtime <= since_seconds:
                changed.append({"path": str(p.relative_to(Path.cwd())), "mtime": mtime})
        except Exception:
            continue
    return {"status": "ok", "recent_changes": changed[:20]}

def main():
    parser = argparse.ArgumentParser(description="Organ IO")
    parser.add_argument("--tool", default="", help="Tool name to execute")
    parser.add_argument("--action", default="", help="Action/Tool alias")
    parser.add_argument("--sense", default="", help="Sense query")
    parser.add_argument("--path", default="", help="File or directory path")
    parser.add_argument("--content", default="", help="File content for write")

    args, unknown = parser.parse_known_args()
    op = (args.tool or args.action or "").lower()

    if not op:
        # Fallback heuristic: if content is supplied -> write, if path only -> read
        if args.content:
            op = "write_file"
        elif args.path:
            op = "read_file"
        elif args.sense:
            op = "sense"

    if op in ("read", "read_file", "cat"):
        res = tool_read(args.path)
    elif op in ("write", "write_file"):
        res = tool_write(args.path, args.content)
    elif op in ("list", "list_files", "ls"):
        res = tool_list(args.path)
    elif op in ("fs_changes", "sense"):
        res = sense_fs_changes()
    else:
        res = {"status": "error", "error": f"Unknown IO operation: {op}"}

    print(json.dumps(res, ensure_ascii=False, indent=2))
    sys.exit(0 if res.get("status") == "ok" else 1)

if __name__ == "__main__":
    main()
