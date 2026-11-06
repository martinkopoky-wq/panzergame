"""Baut aus den Showcase-Frames (panzer showcase <id> <ordner>) die Doku-Videos.

Ergebnis pro Waffe: website/media/waffen/<id>.mp4 (mit Namen eingeblendet) und
website/media/waffen/<id>.jpg (Standbild für die Vorschau).

Aufruf: python3 scripts/encode_videos.py <frames-ordner> [--only ID ...]
"""
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FONT = ROOT / "assets/fonts/RobotoCondensed-Bold.ttf"


def encode(frames: Path, item: dict, out_dir: Path) -> None:
    stem = item["id"].lower()
    title = item["name"].replace(":", r"\:").replace("'", r"\'")
    vf = (
        "scale=800:-2:flags=lanczos,"
        f"drawtext=fontfile='{FONT}':text='{title}':fontcolor=0xffb060:fontsize=30:"
        "box=1:boxcolor=0x0d0e12cc:boxborderw=10:x=(w-text_w)/2:y=14"
    )
    subprocess.run(
        [
            "ffmpeg", "-y", "-loglevel", "error",
            "-framerate", "60", "-i", str(frames / "%04d.png"),
            "-vf", vf,
            "-c:v", "libx264", "-preset", "slow", "-crf", "28", "-pix_fmt", "yuv420p",
            "-movflags", "+faststart", "-an",
            str(out_dir / f"{stem}.mp4"),
        ],
        check=True,
    )
    subprocess.run(
        [
            "ffmpeg", "-y", "-loglevel", "error", "-i", str(frames / "0150.png"),
            "-vf", "scale=800:-2:flags=lanczos", "-q:v", "4",
            str(out_dir / f"{stem}.jpg"),
        ],
        check=True,
    )


def main() -> None:
    frames_root = Path(sys.argv[1])
    only = set(sys.argv[sys.argv.index("--only") + 1 :]) if "--only" in sys.argv else None
    items = json.loads((ROOT / "website/data/waffen.json").read_text(encoding="utf-8"))
    out_dir = ROOT / "website/media/waffen"
    out_dir.mkdir(parents=True, exist_ok=True)
    for item in items:
        if only and item["id"] not in only:
            continue
        frames = frames_root / item["id"]
        if not frames.is_dir() or not any(frames.glob("*.png")):
            print("fehlt:", item["id"])
            continue
        encode(frames, item, out_dir)
        print("ok:", item["id"])


if __name__ == "__main__":
    main()
