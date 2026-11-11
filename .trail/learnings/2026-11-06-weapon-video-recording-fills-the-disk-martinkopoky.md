---
title: Weapon video recording fills the disk with PNG frames
author: Martinkopoky-wq
date: 2026-11-06
standing: true
id: l-9f804c54
scope: repo
status: active
src: init-sessions
touches:
  - scripts/encode_videos.py
---

## Watch out
- Recording 92 weapons wrote several GB of PNG frames into the scratch folder. The disk hit zero free space, eight recordings failed silently, and the shell stopped working.
- In the code: Showcase frames go to /tmp/claude-1000/.../scratchpad/frames. Check `df -h /tmp` first and delete old frame folders after encoding. Rerun failed weapons with `panzer showcase <waffe> <ordner>`, then scripts/encode_videos.py.

_From past Claude sessions here, Nov 6._
