---
title: Weapon labels stay readable when animated text uses fixed cached glyph sizes
author: Martinkopoky-wq
date: 2026-11-11
branch: main
cost: { minutes: 12, tokens: 3046 }
id: l-ecb23f02
scope: repo
status: active
src: agent
touches:
  - crates/client/src/gfx.rs
---

## Decided
Draw game text from a fixed raster size per font and scale the quads for animation. Pre-cache the printable Latin characters and German punctuation at the current DPI before drawing starts. A recorded Geschossweiche pickup turned its name, HUD and other labels into persistent black bars. The same scene became readable after this change.

## Tried and ruled out
Changing the weapon shapes and adding a help strip did not cure the black bars: the first after-recording still had them. The old text helpers rasterized a separate glyph set for every rounded animation size. In the local macroquad 0.4.14 source, Atlas::cache_sprite repacks all glyphs when it grows and Atlas::texture deletes/recreates the texture. That invalidates queued glyph coordinates/texture references. Fixed raster sizes avoid this repeated growth during animated labels.

## Watch out
Keep measuring and drawing on the same raster size and scale. Cache at physical DPI size, since macroquad multiplies the requested font size by DPI internally. Do not undo the owner's existing plain-heading branch in Gfx::fancy. This is separate from the already-shipped Windows frame-presentation fix.

In the code: crates/client/src/gfx.rs uses TEXT_RASTER_SIZE=48 and TITLE_RASTER_SIZE=72, Font::populate_font_cache and TextParams::font_scale. Compare target/weapon-review/before/Switch.png with target/weapon-review/font-check/Switch.png. Reproduce with cargo build --release -p panzer --locked and python3 scripts/weapon_contact_sheet.py Switch. These review artifacts are generated under target and are not committed.
