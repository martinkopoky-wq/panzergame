#!/usr/bin/env python3
"""Schreibt alle Versionen in website/neuigkeiten.html (zwischen den releases-Markern).

Jede Version kommt aus ihrem Tag: RELEASE_NOTES.md in diesem Stand und das Datum des
Tag-Commits. Steht in Cargo.toml eine Version, für die es noch keinen Tag gibt, kommt sie
mit der aktuellen RELEASE_NOTES.md und dem heutigen Datum dazu. So geht die Seite mit dem
Release-Commit online, die Pipeline prüft beim Tag, dass die Version drinsteht.

    python3 scripts/export_releases.py
"""

import datetime
import html
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PAGE = ROOT / "website" / "neuigkeiten.html"
START, END = "<!-- releases:start -->", "<!-- releases:end -->"
MONTHS = ["Januar", "Februar", "März", "April", "Mai", "Juni", "Juli", "August", "September", "Oktober",
          "November", "Dezember"]


def git(*args):
    return subprocess.run(["git", *args], cwd=ROOT, check=True, capture_output=True, text=True).stdout


def version_key(version):
    return tuple(int(part) for part in version.split("."))


def releases():
    """[(version, datum, notizen)], neueste zuerst"""
    found = {}
    for tag in git("tag", "--list", "v*").split():
        if not re.fullmatch(r"v\d+(\.\d+)*", tag):
            continue
        try:
            notes = git("show", f"{tag}:RELEASE_NOTES.md")
        except subprocess.CalledProcessError:
            print(f"{tag}: keine RELEASE_NOTES.md, übersprungen", file=sys.stderr)
            continue
        found[tag[1:]] = (git("log", "-1", "--format=%cs", tag).strip(), notes)
    current = re.search(r'^version = "(.+)"$', (ROOT / "Cargo.toml").read_text(), re.M).group(1)
    if current not in found:
        found[current] = (datetime.date.today().isoformat(), (ROOT / "RELEASE_NOTES.md").read_text())
    return sorted(((v, d, n) for v, (d, n) in found.items()), key=lambda r: version_key(r[0]), reverse=True)


def sections(notes):
    """Markdown der Release-Notizen: "## Titel" und "- Punkt", sonst Zeilen als eigener Punkt"""
    result = []
    for line in notes.splitlines():
        line = line.strip()
        if not line:
            continue
        if line.startswith("## "):
            # "Neu in 2.2.0" -> "Neu", die Version steht schon daneben
            result.append((re.sub(r"\s+in\s+v?\d+(\.\d+)*$", "", line[3:].strip()), []))
            continue
        if not result:
            result.append(("Neu", []))
        result[-1][1].append(line[2:].strip() if line.startswith("- ") else line)
    return result


def render(entries):
    out = []
    for i, (version, date, notes) in enumerate(entries):
        day = datetime.date.fromisoformat(date)
        out += [
            f'<article class="release" id="v{version}">',
            '  <header class="release-head">',
            f'    <h2><a href="#v{version}">{version}</a></h2>',
            f'    <p class="label"><time datetime="{date}">{day.day}. {MONTHS[day.month - 1]} {day.year}</time></p>',
        ]
        if i == 0:
            out.append('    <p class="release-tag">Aktuell</p>')
        out += ["  </header>", '  <div class="release-body">']
        for title, items in sections(notes):
            important = " important" if title.lower().startswith("wichtig") else ""
            out += [f'    <div class="release-part{important}">', f"      <h3>{html.escape(title)}</h3>", "      <ul>"]
            out += [f"        <li>{html.escape(item, quote=False)}</li>" for item in items]
            out += ["      </ul>", "    </div>"]
        out += ["  </div>", "</article>"]
    return "\n".join("        " + line for line in out)


def main():
    page = PAGE.read_text()
    a = page.index(START) + len(START)
    b = page.index(END)
    entries = releases()
    PAGE.write_text(page[:a] + "\n" + render(entries) + "\n        " + page[b:])
    print(f"{PAGE.relative_to(ROOT)}: {', '.join(v for v, _, _ in entries)}")


if __name__ == "__main__":
    main()
