---
title: "Public releases and pushes are done by the owner, not the agent"
author: Martinkopoky-wq
date: 2026-11-06
standing: true
id: l-7c20a177
scope: repo
status: active
src: init-sessions
---

## Decided
- The permission filter blocked the tag push that triggers a public release, and a plain yes was not accepted. The owner types the push command in the prompt, and the agent then verifies the real release.
- In the code: Prepare the commit and tag locally, then give the user `! git push origin <branch> <tag>`. After the workflow finishes, check SHA256SUMS.txt and the latest link. Releases build via a v* tag push.

_From past Claude sessions here, Nov 5._
