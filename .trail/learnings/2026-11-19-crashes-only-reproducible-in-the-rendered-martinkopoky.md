---
title: "Crashes only reproducible in the rendered client: soak-test the headless client, the core fuzz cannot see them"
author: Martinkopoky-wq
date: 2026-11-19
branch: main
cost: { minutes: 41, tokens: 80080 }
id: l-8b04f767
scope: repo
status: active
src: agent
touches:
  - crates/client/src/render/world.rs
  - crates/core/tests/stress_tmp.rs
---

## Decided
A random crash report with no log was found by running the real client headless for a long time: six Docker containers (panzer-headless, Xvfb) each playing `panzer solo 7`, kept without `--rm` so the panic message survives in `docker logs`. One died after ~12 min with index out of bounds in the mystery-box (Wundertüte) spin drawing.

## Tried and ruled out
- Core stress fuzz with ~20k seeds (all rules, players joining/leaving, every item, random-key "monkey" players to fire weapons bots ignore, release + debug overflow checks): no panic. Core logic was fine; the bug was render-only, which no test covers.
- `docker run --rm` loses the log of the crashing container; the first crash was missed that way.

## Watch out
- `a + b % n` binds as `a + (b % n)`; index expressions in render code need the parentheses.
- "Can't open PCM device" / "Audio thread died" panics in the container are just missing sound hardware, not the crash.

In the code: crates/client/src/render/world.rs draw_fx_ui, FxData::Pick mystery spin. Run: docker run -d --init --name pz_N --user $(id -u):$(id -g) -e HOME=/tmp -e RUST_BACKTRACE=1 -v $PWD/target/release/panzer:/panzer:ro panzer-headless timeout 1500 xvfb-run -a -s "-screen 0 1280x800x24" /panzer solo 7
