---
title: On some Windows machines the menu and HUD vanished because the last draw pass was shown before the GPU finished it
author: Martinkopoky-wq
date: 2026-11-08
branch: main
cost: { minutes: 9, tokens: 120212 }
id: l-1cb1a52f
scope: repo
status: active
replaces: l-5ddedfe6
src: agent
touches:
  - crates/client/src/sprites.rs
  - scripts/export_releases.py
  - website/neuigkeiten.html
  - website/neuigkeiten.css
  - RELEASE_NOTES.md
  - crates/client/src/main.rs
  - crates/client/src/modes.rs
---

## Decided
On Windows, every frame ends with a flush plus glFinish before the buffer swap. A player's recording showed the menu, its dark veil and the in-game HUD almost always missing, while the maze behind ran fine at 60 fps. They only flashed up in single frames, each cut off at a different draw call. Cause: the driver shows the image at SwapBuffers before the last pass has run. macroquad only submits the screen-camera pass (veil, menu, HUD) in end_frame, right before the swap. The world pass is submitted earlier and is already finished. Confirmed fixed by the player on 2026-11-08 and shipped in 2.2.1.

## Tried and ruled out
- App-level GL misuse: miniquad picks a double-buffered pixel format, and the passes are spec-compliant.
- More than one swap per frame: there's only one.

## Watch out
- New frame loops must end with app.present().await, not a bare next_frame(), or they lose the fix.
- If it ever comes back on another machine, try sample_count 1 (no MSAA) next and ask for the GPU and driver.

In the code: crates/client/src/main.rs App::present (shot, get_internal_gl().flush(), miniquad::gl::glFinish under cfg!(windows), next_frame). Call sites are in crates/client/src/modes.rs. showcase.rs keeps its own loop (Linux recording only).
