---
title: Style the selected theme explicitly before drawing launcher controls
author: Martinkopoky-wq
date: 2026-11-11
branch: main
id: l-cbcfbae0
scope: repo
status: active
src: agent
touches:
  - PROGRESS.md
  - crates/client/src/gfx.rs
  - crates/client/src/main.rs
  - crates/client/src/modes.rs
  - crates/client/src/render/events.rs
  - crates/client/src/render/hud.rs
  - crates/client/src/render/mod.rs
  - crates/client/src/render/world.rs
  - crates/client/src/showcase.rs
  - crates/client/src/ui.rs
---

## Decided
The owner selected a brighter, playful interface: warm ivory surfaces, dark text, and yellow primary actions. Set egui's visuals for the explicit light theme, matching the theme used by spacing and widgets.

## Tried and ruled out
Calling set_visuals before set_theme(Light) styled the current desktop theme instead. The hand-painted shell looked correct, but the weapon filters still used egui's blue default selection. A rendered weapon-browser review exposed the mismatch.

## Watch out
Use set_visuals_of(Theme::Light, visuals) and style_mut_of(Theme::Light, ...) together. Changing only the shell's constants does not style egui selection, checkboxes, focus, or scrolling controls. Preserve per-frame weapon textures.

In the code: crates/launcher/src/main.rs, Launcher::new; egui 0.36.2 Context::set_visuals targets self.theme(). Review target/ui-review/weapons-filter.png.
