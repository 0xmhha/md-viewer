#!/usr/bin/env python3
"""Count Markdown syntax that mdv's terminal output leaves raw (v1 criterion 2).

The documents are named at run time, so private notes never enter this
repository:

    scripts/check_residue.py --mdv target/release/mdv --vault ~/vault --docs ~/book
    scripts/check_residue.py --mdv target/release/mdv --list paths.txt

--vault DIR   every note under DIR that has a Mermaid block or a callout
--docs DIR    every .md file directly in DIR
--list FILE   one path per line

Each document is rendered with `--color always --width 120`. Code block
frames (from `╭─` to `╰─`) and inline code (SGR 32, green) are dropped, then
the rest is searched for `$$`, a ```mermaid fence, `[[`, `> [!` and a closed
`==text==` highlight. A lone `==` is a comparison written in prose; Obsidian
shows it as is, so it is not counted.

Exit status is 1 when any document has residue, so CI or a release script
can use it. The vault's git commit is printed, because the vault keeps
changing.
"""

import argparse
import re
import subprocess
import sys
from pathlib import Path

TOKENS = ["$$", "```mermaid", "[[", "> [!"]
HIGHLIGHT = re.compile(r"==[^\s=](?:[^=]*[^\s=])?==")
INLINE_CODE = re.compile(r"\x1b\[32m.*?\x1b\[0m")
ESCAPES = re.compile(r"\x1b\[[0-9;]*[A-Za-z]|\x1b\]8;[^\x1b]*\x1b\\|\x1b_G[^\x1b]*\x1b\\")
MERMAID_OR_CALLOUT = re.compile(r"^\s*(>\s*)*(```+|~~~+)\s*mermaid|^\s*>\s*\[!", re.M)


def vault_notes(root: Path) -> list[Path]:
    notes = []
    for path in root.rglob("*.md"):
        if any(part.startswith(".") for part in path.relative_to(root).parts):
            continue
        try:
            if MERMAID_OR_CALLOUT.search(path.read_text(encoding="utf-8")):
                notes.append(path)
        except (OSError, UnicodeDecodeError):
            continue
    return notes


def residue(mdv: str, path: Path) -> list[tuple[int, str, str]]:
    out = subprocess.run(
        [mdv, "--color", "always", "--width", "120", str(path)],
        capture_output=True, text=True, timeout=60,
    ).stdout
    hits, in_block = [], False
    for number, line in enumerate(out.splitlines(), 1):
        body = ESCAPES.sub("", line).lstrip("│ ")
        if body.startswith("╭─"):
            in_block = True
            continue
        if body.startswith("╰─"):
            in_block = False
            continue
        if in_block:
            continue
        text = ESCAPES.sub("", INLINE_CODE.sub("", line))
        hits += [(number, t, text.strip()[:140]) for t in TOKENS if t in text]
        if HIGHLIGHT.search(text):
            hits.append((number, "==text==", text.strip()[:140]))
    return hits


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Count Markdown syntax that mdv's terminal output leaves raw.")
    parser.add_argument("--mdv", default="target/release/mdv")
    parser.add_argument("--vault", type=Path)
    parser.add_argument("--docs", type=Path, action="append", default=[])
    parser.add_argument("--list", type=Path)
    args = parser.parse_args()

    paths: set[Path] = set()
    if args.vault:
        paths.update(vault_notes(args.vault))
        commit = subprocess.run(["git", "-C", str(args.vault), "rev-parse", "--short", "HEAD"],
                                capture_output=True, text=True).stdout.strip()
        dirty = subprocess.run(["git", "-C", str(args.vault), "status", "--porcelain"],
                               capture_output=True, text=True).stdout.count("\n")
        print(f"vault commit {commit or 'unknown'}, {dirty} uncommitted files")
    for folder in args.docs:
        paths.update(folder.glob("*.md"))
    if args.list:
        paths.update(Path(p) for p in args.list.read_text().splitlines() if p.strip())
    if not paths:
        parser.error("give --vault, --docs or --list")

    bad = 0
    for path in sorted(paths):
        hits = residue(args.mdv, path)
        if hits:
            bad += 1
            print(f"## {path}")
            for number, token, text in hits:
                print(f"  {number}: {token!r}  {text}")
    print(f"checked {len(paths)} documents, {bad} with residue")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
