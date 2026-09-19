#!/usr/bin/env python3
"""The specification's code blocks survive an editorial pass.

Compares every ```nika block of the three parts in the working tree with the
same file at a git revision (default HEAD): same count, same order, same
bytes. Prose may change freely; the blocks are the tests.

    python3 scripts/check-spec-blocks.py [REV]
"""
import subprocess, sys, pathlib

PARTS = ["10-nikaia-light.md", "20-nikaia-advance.md", "30-nikaia-tooling.md"]
ROOT = pathlib.Path(__file__).resolve().parent.parent / "docs" / "specification"

def blocks(text):
    out, lines, at = [], text.split("\n"), 0
    while at < len(lines):
        if lines[at].strip() == "```nika":
            end = at + 1
            while end < len(lines) and lines[end].strip() != "```":
                end += 1
            out.append("\n".join(lines[at + 1:end]))
            at = end + 1
        else:
            at += 1
    return out

rev = sys.argv[1] if len(sys.argv) > 1 else "HEAD"
bad = 0
for part in PARTS:
    was = subprocess.run(["git", "show", f"{rev}:docs/specification/{part}"],
                         capture_output=True, text=True, check=True).stdout
    now = (ROOT / part).read_text()
    a, b = blocks(was), blocks(now)
    if len(a) != len(b):
        print(f"{part}: {len(a)} blocks at {rev}, {len(b)} now"); bad += 1
    for i, (x, y) in enumerate(zip(a, b), 1):
        if x != y:
            print(f"{part} #{i}: block changed\n--- {rev}\n{x[:200]}\n--- now\n{y[:200]}"); bad += 1
            break
    if not bad:
        print(f"{part}: {len(b)} blocks unchanged")
sys.exit(1 if bad else 0)
