---
title: "The website news page is built from the release notes in git, not from the GitHub API"
author: Martinkopoky-wq
date: 2026-11-08
branch: main
cost: { minutes: 9, tokens: 122692 }
id: l-1aab0a36
scope: repo
status: active
src: agent
touches:
  - crates/client/src/sprites.rs
  - scripts/export_releases.py
  - website/neuigkeiten.html
  - website/neuigkeiten.css
  - RELEASE_NOTES.md
  - Cargo.toml
  - ".github/workflows/release.yml"
---

## Decided
The news page lists every release, generated as static HTML from RELEASE_NOTES.md at each v* tag plus the not-yet-tagged version from Cargo.toml. Fetching releases from the GitHub API in the browser was dropped: unauthenticated it allows 60 requests per hour per IP, and players often share one IP (school network), so the page would go blank. Static HTML also works without JavaScript, like the rest of the site.

## Watch out
- The page has to be regenerated with every release, after writing RELEASE_NOTES.md. The release workflow fails on the tag if the version is missing from the page.
- Pages only deploys from main and only on changes in website/, so the new entry goes live with the release commit, a few minutes before the release binaries.
- "## Neu in 2.2.0" headings are shown without the version suffix, and a heading starting with "Wichtig" is highlighted.

In the code: python3 scripts/export_releases.py writes website/neuigkeiten.html between the releases:start/end markers. The check is in .github/workflows/release.yml, step "Version prüfen" (grep for id="v<version>"). Styles are in website/neuigkeiten.css.
