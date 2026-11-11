---
title: "Bots must remember targets by player slot, not list position"
author: Martinkopoky-wq
date: 2026-11-06
standing: true
id: l-c16d4095
scope: repo
status: active
src: init-sessions
touches:
  - crates/core/src/bot.rs
  - crates/core/tests/fuzz.rs
  - crates/server/tests/net.rs
---

## Watch out
- Bots stored their target as a position in the tank list. When a player left, the list shifted and the server crashed, or the bot silently measured against the wrong tank.
- In the code: In crates/core/src/bot.rs keep Bot.target as a slot id, never an index into Game::tanks. remove_player shifts the Vec. Tests are in crates/core/tests/fuzz.rs and crates/server/tests/net.rs.

_From past Claude sessions here, Nov 5._
