---
title: GitHub-Runner-Container trafen am 11.11.2026 keinen DNS mehr
author: Martinkopoky-wq
date: 2026-11-11
branch: main
id: l-0a978398
scope: repo
status: active
src: agent
touches:
  - ".github/workflows/release.yml"
  - Cargo.toml
---

## Decided
- Release-Run v2.3.0 scheiterte dreimal in Folge an 'apt: Could not resolve archive.ubuntu.com' inside dem Job-Container (ubuntu:22.04) auf GitHub-Hosted Runnern; macOS/Windows-Jobs ohne Container waren grün, neue Runner-Reruns halfen nicht.
- release.yml härtet deshalb beide Container-Jobs: 'options: --dns 1.1.1.1 --dns 8.8.8.8' und apt update+install in einer Wiederholungsschleife (5 Versuche, steigende Pause). Nicht nochmal blind rerunen, wenn 'Could not resolve' im Container auftritt.
- Der Tag darf für so einen Pipeline-Fix auf einen Folgecommit gleicher Version gezeigt werden (git tag -f + Force-Push durch den Owner), weil der Release-Inhalt (Cargo.toml, RELEASE_NOTES) unverändert bleibt.

## Evidence
- runs 43034034603 attempts 1-3, jobs 130568098721/130573773439/130597219722, exit 100 in Schritt 'Pakete'.
