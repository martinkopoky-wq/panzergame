---
title: "The ammo number of single-charge weapons in the snapshot counts free normal bullets, so the client spots instant-weapon use by the weapon disappearing"
author: Martinkopoky-wq
date: 2026-11-12
branch: main
cost: { minutes: 27, tokens: 454411 }
id: l-d15cb936
scope: repo
status: active
src: agent
touches:
  - crates/client/src/render/events.rs
  - crates/client/src/audio.rs
  - crates/core/src/game/mod.rs
  - crates/client/src/render/weapons.rs
---

## Decided
- Instant weapons (Geschossschere, Entwaffnungsfächer, Zündpause, Konterklinge) create no shot and no fx, so the client infers their use from the tank view between two frames. With more than one charge the ammo drops. With one charge the weapon is gone while no box with that item lies at the tank (a box there means the tank was disarmed). A mystery box replacing the weapon is not a use.

## Tried and ruled out
- Checking "ammo was 1 before" for single-charge weapons never fired: for them the transmitted ammo is the number of free normal bullets (e.g. 5), not the weapon's charge.

## Watch out
- Any client feature that reads TankView.ammo must check Item::has_ammo first.

In the code: crates/core/src/game/mod.rs reload_state (what ammo means on the wire), crates/client/src/render/weapons.rs instant_used / detect_action / draw_actions, tests in the same file.
