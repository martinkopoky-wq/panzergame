# Panzer UI

The owner asked for a brighter, more playful launcher and game. The first pass mostly changed colors and left the old hierarchy intact; the owner rejected it as just light mode. The replacement changes navigation, match setup, arena presentation, and the gameplay HUD.

## Direction and audit

- **Product:** quick tank matches on one keyboard or over LAN and the internet.
- **Character:** tactile, lively, direct.
- **World:** a tabletop arena with solid equipment controls.
- **Moments:** a real live arena preview and a prominent personal loadout.
- **Signature:** three small tank tread marks identify the selected control or primary action.

The old game menu put name, host, address, join, local player buttons, bots, and rules in one long form. Hosting appeared first even for someone who wanted a solo match. Equal HUD cards gave the player's current weapon little room. The launcher kept release notes next to the main action and confined the weapon browser to the same small area. A different palette did not address those problems.

## What changed

1. **Spacing:** 40px outer game margins, consistent panel padding, 40px inputs and a 48px game start action. The launcher uses a persistent 176px sidebar and a separate launch area.
2. **Color:** ivory setup surfaces, dark arena framing and equipment strip, yellow primary actions. Player colors retain their gameplay meaning. The arena remains the richest visual.
3. **Icons:** the existing tank sprites illustrate modes and player slots. Existing logo and artwork remain; no extra icon family or generated imagery.
4. **Copy:** short mode names, contextual instructions, and action labels that say what starts. Online name and IP errors appear next to the affected setup.
5. **Type:** keep the bundled font families and fixed glyph raster/cache implementation. Weapon names, ammunition and scores have separate space; long names fit safely at Unicode character boundaries.
6. **Layout:** four mode choices precede setup. Solo, shared keyboard, hosting and joining show only their relevant controls. The full live arena has its own viewport. Solo/online HUDs give the local player a 272px loadout panel; shared keyboard play gives humans more width than bots. The launcher has separate play, weapon and release-note pages.
7. **Details:** clipped equipment corners, tread marks, keyboard keycaps, visible field focus and input tails that keep the caret visible. Launcher actions keep egui's native input semantics.
8. **Visuals:** show actual gameplay and the existing illustration. Weapon rows pair a larger preview with instructions and ammunition. Only visible rows decode textures, and each animation frame remains a separate texture.
9. **Motion:** retain useful gameplay animation and live previews; remove decorative title pulsing, glowing buttons and score bounce. A short score marker indicates a point change.

## Compatibility

`crates/client/src/theme.rs` owns game UI colors and drawing helpers; the launcher mirrors the palette in egui. Configure the light theme explicitly with `set_visuals_of` and `style_mut_of`, even though some surfaces are painted dark. Otherwise the desktop's theme choice can leave default selection colors behind.

Keep `App::present().await` in every game frame loop for the verified Windows GPU flush fix. The arena preview restores the screen camera and clears its viewport setting before returning to normal gameplay. No item numbers, physics, protocol layout, or update backend changed. The previous weapon rendering and hint improvements remain intact.

For development, `target/debug/panzer-launcher --local` starts the sibling `panzer` binary rather than an installed public release. This makes it possible to review the rebuilt UI from the launcher's play button. Normal launcher behavior still uses the installed game, update checks, downloads and repair. `--docs` still opens the weapon browser directly.

## Verification

The native binaries build with the locked, offline dependencies. The workspace suite passes **68 tests**, with **3 intentionally ignored**. Review rounds consisted of the original screen, a layout pass, and a final interaction/visual check. The first layout review caught an overly dark illustration caused by texture tinting; the final version uses white tint.

`scripts/ui_smoke.py` runs native X11 input against an isolated Xvfb display with temporary game and launcher settings. It checks online-name and empty-address validation, solo start, saved name/bots/rule, three players on one keyboard, return to the menu, hosting, the player list, chat, an eight-player HUD, animated weapon frames, the effect filter, release-note navigation, the 860×540 minimum size, and launching the current local game. Hosting is skipped explicitly if UDP 5555 is already occupied. Screenshots also require visual review; Linux checks do not replace a Windows driver check.

With the existing local `panzer-headless` image and host tools `xwd`, `xwininfo`, `ffmpeg`, Python/Pillow, libX11 and libXtst:

```sh
cargo build -p panzer -p panzer-launcher --locked --offline
cargo test --workspace --locked --offline
docker run --rm --init -d --name panzer-ui-review \
  -v /tmp/.X11-unix:/tmp/.X11-unix \
  panzer-headless Xvfb :119 -screen 0 1280x800x24 -nolisten tcp -ac
python3 scripts/ui_smoke.py --display :119
docker stop panzer-ui-review
```

The script also accepts any dedicated, already-running Xvfb display. `--output <directory>` changes the artifact location. Default artifacts and the JSON report live in `target/ui-redesign/verified/`; baseline and first-pass captures live alongside them. These files are intentionally untracked. Tests stop only their own processes and do not click on the owner's desktop.

Design process adapted from **unslop by Damir Sokolovsky ([@heydamir](https://x.com/heydamir))**, [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/).
