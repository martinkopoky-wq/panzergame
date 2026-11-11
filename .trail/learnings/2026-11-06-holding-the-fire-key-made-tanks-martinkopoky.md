---
title: Holding the fire key made tanks drive forward because of a name clash
author: Martinkopoky-wq
date: 2026-11-06
standing: true
id: l-bf9c4d9c
scope: repo
status: active
src: init-sessions
touches:
  - panzer.py
---

## Watch out
- Two constants called FIRE existed, a key bit and a particle type. The later one won, and its value 1 equals the forward bit, so holding fire drove the tank.
- In the code: In the Python version (panzer.py) the key bit is now SHOOT. Avoid reusing names across range() enum blocks and key-bit constants. The Rust port is separate.

_From past Claude sessions here, Oct 19._
