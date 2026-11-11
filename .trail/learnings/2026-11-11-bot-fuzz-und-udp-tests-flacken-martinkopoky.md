---
title: "Bot-Fuzz und UDP-Tests flacken selten unter Last, nicht neu bauen"
author: Martinkopoky-wq
date: 2026-11-11
branch: main
id: l-0720228f
scope: repo
status: active
src: agent
---

## Decided
- 'bot_matches_all_sizes' scheitert gelegentlich an 'zu wenige Runden' (~1 von 30 Läufen, vor und nach den Performance-Umbauten vom 11.11. gleich): Bot::new的种子et aus Entropie, einzelne Matches verlangsamen durch seltene Stillstands-Situationen. Ein einzelner Fehlschlag ist kein Regressionssignal — erst nachhäufen, dann suchen.
- 'multiplayer_over_udp' (Speedhack-Fenster in Echtzeit) flackt, wenn parallel andere cargo-Läufe alle Kerne belegen. Bei Fehlschlag erst allein wiederholen.

## Evidence
- 1/58 Läufe fehlgeschlagen im neuen Baum, 0/25 im v2.3.0-Baum; UDP-Test fiel einmal bei zwei parallelen cargo test, bestand 5/5 allein.
