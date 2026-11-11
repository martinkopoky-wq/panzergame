---
title: Releases nur mit Bugfixes und Performance erhöhen die letzte Zahl
author: Martinkopoky-wq
date: 2026-11-11
branch: main
id: l-b07422a3
scope: repo
status: active
src: agent
touches:
  - Cargo.toml
  - scripts/export_releases.py
  - RELEASE_NOTES.md
---

## Decided
- Die Versionsnummer in Cargo.toml (workspace.package.version, gepflegt mit scripts/export_releases.py und RELEASE_NOTES.md) folgt strikt semantischer Version nach Wunsch des Owners: Änderungen ohne neue Spieleigenschaften (Bugfixes, Performance, Pipeline) erhöhen nur die letzte Zahl, x.x.n -> x.x.(n+1). Neue Features oder neue Waffen: mittlere Zahl.
- Beispiel 11.11.2026: die Performance-Runde nach v2.3.0 erschien als v2.3.1.

## Why
- Der Owner entscheidet die Versionsnummer; Agenten sollen aus einer Bugfix-Runde kein 2.4.0 machen.
