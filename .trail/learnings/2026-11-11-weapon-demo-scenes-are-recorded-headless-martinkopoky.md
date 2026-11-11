---
title: "Weapon demo scenes are recorded headless in a Docker container with Xvfb, not in a window on the desktop"
author: Martinkopoky-wq
date: 2026-11-11
branch: main
cost: { minutes: 1, tokens: 2373 }
id: l-c53d2720
scope: repo
status: active
src: agent
touches:
  - crates/client/src/showcase.rs
  - scripts/encode_videos.py
---

## Decided
Showcase recordings (crates/client/src/showcase.rs, `panzer showcase <Item> <dir>`, frames fed to scripts/encode_videos.py) run inside a Debian trixie container with Xvfb and Mesa software GL. The host-built release binary is mounted read-only. No window opens on the owner's desktop, and 300 frames take about 7 s. Debian trixie matches the host glibc, so the host build runs unchanged.

## Tried and ruled out
- Starting the game on a hidden Hyprland special workspace (`hyprctl dispatch exec "[workspace special:x silent] ..."`): the window never gets frames, 0 PNGs after 40 s.
- `xvfb-run` as PID 1 in the container without `--init`: it hangs forever in sigsuspend, because the readiness signal from Xvfb never arrives.

## Watch out
- "Audio thread died" in the log is harmless (no sound device in the container).
- Frames are 1000x680. Delete the PNG frames right after building contact sheets, /tmp is a tmpfs.

In the code: showcase::run in crates/client/src/showcase.rs writes the PNGs. Image = FROM debian:trixie + apt xvfb xauth libgl1 libgl1-mesa-dri libegl1 libglx-mesa0 libasound2t64 libx11-6 libxi6 libxcursor1 libxrandr2 libxkbcommon0 ffmpeg. Run: docker run --rm --init --user $(id -u):$(id -g) -e HOME=/tmp -v <panzer>:/panzer:ro -v <out>:/out panzer-headless xvfb-run -a -s "-screen 0 1280x800x24" /panzer showcase <Item> /out/<Item>. Contact sheet: ffmpeg -i %04d.png -vf "select='not(mod(n\,17))',scale=640:-1,tile=3x3".
