---
title: "Shot trails, bounce sparks and flight directions vanished in 2.3.1 because the shot history kept dead shots instead of live ones"
author: Martinkopoky-wq
date: 2026-11-12
branch: main
cost: { minutes: 27, tokens: 454411 }
id: l-27d5b7ee
scope: repo
status: active
src: agent
touches:
  - crates/client/src/render/events.rs
  - crates/client/src/audio.rs
---

## Watch out
- The client keeps a short position history per shot. It drives the glowing trails, bounce sparks and sounds, and the heading of shots without an angle in the snapshot (fireworks, Querschläger, Enterhaken, Kanonenkugel smear). The 2.3.1 performance change replaced a HashSet with a sorted id list and inverted the retain check, so every live shot lost its history each frame. Nothing crashed and no test failed, it only looked flat.
- Fixed by keeping ids that are found in the sorted live list, with a unit test.

In the code: crates/client/src/render/events.rs keep_live and track_shots, test history_survives_only_for_live_shots. Introduced in commit b11e670.
