---
title: Adding weapons before existing ones shifts item numbers and breaks old clients
author: Martinkopoky-wq
date: 2026-11-06
standing: true
id: l-ef578a3c
scope: repo
status: active
src: init-sessions
touches:
  - crates/core/src/items.rs
  - RELEASE_NOTES.md
---

## Watch out
- Batches 2 to 4 inserted new items before Shield, which moved the item numbers on the wire. Old versions could no longer play with new ones, and the release note claiming compatibility became wrong.
- In the code: Whenever items are inserted in crates/core/src/items.rs, bump PROTO (now 11) and fix the compatibility line in RELEASE_NOTES.md. Version 2.2.0 does not interoperate with 2.1.2 or older.

_From past Claude sessions here, Nov 6._
