#!/usr/bin/env python3
"""Every `ADR-0NN Dn` / `ADR-0NN §x` citation must point at a heading that exists.

A decision is only citable if the number is stable, so a dangling `D7` is a
defect in the same way a dangling link is. Run from the repository root; exits
non-zero and lists what is dangling.
"""

import glob
import os
import re
import sys

ADR_DIR = "docs/specification/adr"

SEARCH = [
    "crates/**/*.rs",
    "crates/**/*.nika",
    "tests/**/*",
    "examples/**/*",
    "benches/**/*",
    "docs/**/*.md",
    "scripts/**/*",
    "README.md",
    "manifesto.md",
]


def headings():
    """What each ADR actually offers: its `Dn` and its top-level section numbers."""
    have = {}
    for path in sorted(glob.glob(f"{ADR_DIR}/adr-*.md")):
        num = re.search(r"adr-(\d{3})\.md$", path).group(1)
        text = open(path, encoding="utf-8").read()
        ds = set(re.findall(r"^#{2,4}\s+(D\d+)\b", text, re.M))
        # `## 3. Consequences` and `### 4.2 — …` both name a section.
        secs = set(re.findall(r"^#{2,4}\s+(\d+(?:\.\d+)*)[.\s]", text, re.M))
        have[num] = (ds, secs)
    return have


def citations():
    """Every (adr, label) pair referenced anywhere, with the file that does it."""
    seen = []
    files = set()
    for pattern in SEARCH:
        files |= {f for f in glob.glob(pattern, recursive=True) if os.path.isfile(f)}
    for f in sorted(files):
        if os.path.abspath(f) == os.path.abspath(__file__):
            continue
        try:
            text = open(f, encoding="utf-8", errors="ignore").read()
        except OSError:
            continue
        # Two spellings reach the same place: a bare `ADR-023 D7`, and the
        # markdown form `[ADR-023](adr-023.md) D7` that every ADR header uses.
        for pattern in (
            r"ADR[- ]?(\d{3})((?:[^A-Za-z0-9]{0,4}(?:D\d+|§[\d.]+))+)",
            r"\]\(adr-(\d{3})\.md\)((?:[^A-Za-z0-9]{0,4}(?:D\d+|§[\d.]+))+)",
        ):
            for m in re.finditer(pattern, text):
                for label in re.findall(r"D\d+|§[\d.]+", m.group(2)):
                    seen.append((m.group(1), label, f))
    return seen


def main():
    have = headings()
    bad = []
    for adr, label, where in citations():
        if adr not in have:
            bad.append((adr, label, where, "no such ADR"))
            continue
        ds, secs = have[adr]
        if label.startswith("D"):
            if label not in ds:
                bad.append((adr, label, where, "no such decision"))
        else:
            sec = label.lstrip("§").rstrip(".")
            if sec and sec not in secs:
                bad.append((adr, label, where, "no such section"))

    if bad:
        print(f"{len(bad)} dangling ADR citation(s):")
        for adr, label, where, why in sorted(set(bad)):
            print(f"  {where}: ADR-{adr} {label} — {why}")
        return 1
    print(f"all ADR citations resolve ({len(citations())} checked)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
