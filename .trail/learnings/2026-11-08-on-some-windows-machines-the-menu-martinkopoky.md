---
title: On some Windows machines the menu and HUD vanish because the last draw pass is shown before the GPU finishes it
author: Martinkopoky-wq
date: 2026-11-08
branch: main
cost: { minutes: 9, tokens: 220840 }
id: l-5ddedfe6
scope: repo
status: active
src: agent
touches:
  - crates/client/src/main.rs
  - crates/client/src/modes.rs
---

## Decided
Every frame now ends with an explicit flush plus glFinish on Windows before the buffer swap. A player's recording showed the main menu almost always invisible and the in-game HUD missing, while the maze and tanks behind them rendered fine at 60 fps. The menu only flashed up in single frames, each time cut off at a different draw call (sometimes after "BOTS DAZU", sometimes after "2 Spieler"). The dark veil over the menu backdrop was missing too.

That pattern means the image gets presented while the GPU is still working on the last pass. The world pass is flushed early (at the switch to the screen camera) and is finished by the time of the swap. Everything drawn with the screen camera (veil, menu, HUD, banners) is only submitted by macroquad's end_frame right before SwapBuffers, so the driver shows the image before those commands have run. It's a driver/compositor sync problem on that machine, not menu logic; Linux is fine.

## Tried and ruled out
- App-level GL misuse: miniquad picks a double-buffered pixel format, and macroquad's passes and camera switches are spec-compliant.
- Double next_frame or extra awaits per frame: there's only one swap per frame.

## Watch out
- Not yet confirmed on the affected machine. If it still flickers, try sample_count 1 (no MSAA) next and ask for the GPU and driver (hybrid laptop GPU, or the OpenGL compatibility layer on ARM/VMs are suspects).
- New frame loops must end with app.present().await, not a bare next_frame(), or they lose the fix.

In the code: crates/client/src/main.rs App::present (shot + get_internal_gl().flush() + miniquad::gl::glFinish under cfg!(windows) + next_frame), call sites in crates/client/src/modes.rs. showcase.rs keeps its own loop (Linux recording only).
