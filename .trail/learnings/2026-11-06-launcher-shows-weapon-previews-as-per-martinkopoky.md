---
title: "Launcher shows weapon previews as per-frame images, not sprite sheet cropping"
author: Martinkopoky-wq
date: 2026-11-06
standing: true
id: l-a9324a19
scope: repo
status: active
src: init-sessions
touches:
  - scripts/build_sprites.py
---

## Tried and ruled out
- One huge sprite sheet exceeded texture limits, and cropping with UV rectangles rendered as a flat strip in this egui version. It now splits each weapon into its own file and loads frames as separate images.
- In the code: Previews live in assets/waffen/rows, built by scripts/build_sprites.py. Don't use UV cropping in the egui launcher docs view. Open it with `panzer-launcher --docs` to check.

_From past Claude sessions here, Nov 6._
