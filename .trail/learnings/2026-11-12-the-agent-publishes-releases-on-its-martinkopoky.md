---
title: "The agent publishes releases on its own, including pushing main and the release tag"
author: Martinkopoky-wq
date: 2026-11-12
branch: main
cost: { minutes: 5, tokens: 45097 }
id: l-0f0dd922
scope: repo
status: active
replaces: l-7c20a177
src: agent
touches:
  - Cargo.lock
  - Cargo.toml
  - RELEASE_NOTES.md
  - website/neuigkeiten.html
  - PROGRESS.md
  - crates/client/src/audio.rs
  - crates/client/src/gfx.rs
  - crates/client/src/particles.rs
  - crates/client/src/render/events.rs
  - crates/client/src/render/mod.rs
---

## Decided
- On 2026-11-12 the owner said releases may be done completely autonomously: version bump, release notes, news page, commit, pushing main and the v* tag, watching the workflow and checking the real release. Earlier the permission filter had blocked the tag push and the owner pushed by hand; that is no longer the rule.
- The steps live in the personal skill panzer-release, including a script that checks the published release.

## Watch out
- If the permission filter blocks the push again, hand the owner the exact push line and continue with the checks afterwards.

In the code: ~/.claude/skills/panzer-release/SKILL.md and verify_release.sh, .github/workflows/release.yml (tag v* publishes), scripts/export_releases.py.
