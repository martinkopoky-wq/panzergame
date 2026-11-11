---
title: A brighter interface still needs a different hierarchy and flow
author: Martinkopoky-wq
date: 2026-11-11
branch: main
id: l-dbc50d44
scope: repo
status: active
src: agent
touches:
  - PROGRESS.md
  - crates/client/src/gfx.rs
  - crates/client/src/main.rs
  - crates/client/src/modes.rs
  - crates/client/src/render/hud.rs
  - crates/client/src/render/mod.rs
  - crates/client/src/ui.rs
  - crates/launcher/src/main.rs
---

## Decided
Treat the brighter, playful UI request as a change to hierarchy and interaction. The owner rejected the first pale recoloring because the interface still worked and read like the old one. Put mode choice before match setup, show only relevant settings, frame the actual arena, and give the player's equipment a clear place. Keep the launcher organized around play, weapons, and news.

## Tried and ruled out
Ivory surfaces, yellow actions, and tidier spacing alone were insufficient. Do not describe a palette change as completion of this redesign. Use native screenshots and actual input to compare the flow and information hierarchy, including the minimum launcher size and a full HUD.

## Watch out
The normal launcher can start a cached public build, hiding local UI changes. Use the explicit development mode when reviewing local work. Preserve the Windows frame presentation fix and individual weapon-frame textures while changing layout. A dark desktop preference still needs explicit styling of egui's selected light theme, including selection colors.

In the code:
- `crates/client/src/modes.rs`: four mode choices and contextual setup.
- `crates/client/src/render/hud.rs`: larger personal loadout and compact opponents.
- `crates/launcher/src/main.rs`: navigation pages and `--local` sibling executable selection.
- `scripts/ui_smoke.py`, `docs/UI_DESIGN.md`: native interaction checks and reproduction commands.
