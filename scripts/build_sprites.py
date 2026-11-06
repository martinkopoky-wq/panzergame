"""Baut das Sprite-Blatt für die Waffen-Doku im Launcher.

Pro Waffe 12 Bilder aus der Aufnahme (alle 20 Frames), verkleinert auf 160×109. Die
Zeilen stehen in der Reihenfolge von website/data/waffen.json. Der Launcher spielt eine
Zeile als kurze Animation ab.

Ergebnis: assets/waffen/rows/NNN.jpg, eine Datei pro Waffe (Reihenfolge wie im JSON),
plus crates/launcher/src/waffen_rows.rs, das alle Dateien einbindet. Eine Datei pro Waffe
statt einem großen Blatt, weil eine einzelne Textur von 10000 px Höhe nicht darstellbar ist.

Aufruf: python3 scripts/build_sprites.py <frames-ordner>
"""
import json
import sys
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
# Anzeige im Launcher: 320x218 px. Jedes 6. Bild (60 fps Aufnahme) = 10 Bilder je Sekunde
# ergibt Echtzeit. 49 Bilder decken die ersten 4,9 s ab.
FRAME_W, FRAME_H = 320, 218
COLS = 49
STEP = 6
FPS = 10.0


def strip(frames: Path) -> Image.Image:
    out = Image.new("RGB", (FRAME_W * COLS, FRAME_H))
    for col in range(COLS):
        p = frames / f"{col * STEP:04d}.png"
        img = Image.open(p).convert("RGB").resize((FRAME_W, FRAME_H), Image.LANCZOS)
        out.paste(img, (col * FRAME_W, 0))
    return out


def main() -> None:
    frames_root = Path(sys.argv[1])
    items = json.loads((ROOT / "website/data/waffen.json").read_text(encoding="utf-8"))
    out = ROOT / "assets/waffen/rows"
    out.mkdir(parents=True, exist_ok=True)
    for old in out.glob("*.jpg"):
        old.unlink()
    names = []
    for i, item in enumerate(items):
        frames = frames_root / item["id"]
        if not frames.is_dir():
            raise SystemExit(f"Frames fehlen: {item['id']}")
        name = f"{i:03d}.jpg"
        strip(frames).save(out / name, quality=80)
        names.append(name)
    old_sheet = ROOT / "assets/waffen/strips.jpg"
    if old_sheet.exists():
        old_sheet.unlink()
    lines = [
        "// Erzeugt von scripts/build_sprites.py. Nicht von Hand ändern.",
        f"pub const COLS: usize = {COLS};",
        f"pub const FPS: f64 = {FPS:.1f};",
        "pub const ROWS: &[&[u8]] = &[",
    ]
    lines += [f'    include_bytes!("../../../assets/waffen/rows/{n}"),' for n in names]
    lines.append("];")
    (ROOT / "crates/launcher/src/waffen_rows.rs").write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"{len(names)} Zeilen -> {out}")


if __name__ == "__main__":
    main()
