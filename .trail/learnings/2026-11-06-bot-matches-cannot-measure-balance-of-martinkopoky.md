---
title: "Bot matches cannot measure balance of new weapons, so tuning waits for humans"
author: Martinkopoky-wq
date: 2026-11-06
standing: true
id: l-10c7b8df
scope: repo
status: active
src: init-sessions
touches:
  - crates/core/tests/balance.rs
---

## Decided
- Bots barely use the new weapons, so match stats were meaningless. Per-weapon bot matches showed outliers, but the owner chose to adjust only after human playtests.
- In the code: Rough tool: crates/core/tests/balance.rs with --ignored. Strong: Stillstandsurteil, homing missiles. Do not rebalance from these numbers alone. Bots do not know the new weapons.

_From past Claude sessions here, Nov 6._
