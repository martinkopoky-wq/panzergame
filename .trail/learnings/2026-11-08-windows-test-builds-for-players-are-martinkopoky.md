---
title: "Windows test builds for players are cross-compiled locally in Docker, no push or release needed"
author: Martinkopoky-wq
date: 2026-11-08
branch: fix/windows-flicker
cost: { minutes: 6, tokens: 33233 }
id: l-0e967669
scope: repo
status: active
src: agent
touches:
  - crates/client/src/main.rs
  - crates/client/src/modes.rs
---

## Decided
To get a fix to a Windows player quickly, build panzer.exe on Linux with the MinGW toolchain in Docker and send the zip directly. The game is a single exe (fonts and sprites are built in) that only imports Windows system DLLs, so it runs without the launcher or installer. Releases and pushes stay with the owner. The CI route (workflow_dispatch on a branch) gives the exact MSVC build but needs a push.

## Watch out
- The MinGW build differs from the MSVC release build. That's fine for graphics or gameplay tests, not as a release.
- The exe is unsigned, so the player has to click through SmartScreen ("More info", then "Run anyway").
- The player should start the exe directly, not drop it into the launcher's game folder.

In the code: image = FROM rust:1.95-bookworm + apt gcc-mingw-w64-x86-64 + rustup target add x86_64-pc-windows-gnu. Build from `git archive HEAD` with CARGO_HOME/CARGO_TARGET_DIR in a scratch folder, run as the user (--user uid:gid): cargo build --release --locked -p panzer --target x86_64-pc-windows-gnu (~40 s). Check imports with x86_64-w64-mingw32-objdump -p panzer.exe | grep "DLL Name". Zipped size ~1.5 MB. The icon_canvas dead_code warning only appears on the Windows target and was there before.
