"""Record real weapon scenes without opening a desktop window.

Requires Claude's local panzer-headless Docker image and ffmpeg on the host.
PNG frames live in a TemporaryDirectory and are removed even when encoding fails.

  cargo build --release -p panzer --locked
  python3 scripts/weapon_contact_sheet.py Ring Hammer Pendel Rust
  python3 scripts/weapon_contact_sheet.py --binary /tmp/panzer-before --output target/weapon-review/before Ring
"""

import argparse
from concurrent.futures import ThreadPoolExecutor, as_completed
import json
import os
from pathlib import Path
import subprocess
import tempfile


def run(command):
    result = subprocess.run(command, capture_output=True, text=True, timeout=120)
    if result.returncode:
        raise RuntimeError(f"{command[0]} failed ({result.returncode}):\n{result.stderr[-6000:]}")


def record(binary, item, output, docker_image):
    # Only this invocation's disposable frames are ever removed.
    with tempfile.TemporaryDirectory(prefix="panzer-weapon-") as scratch:
        frames = Path(scratch) / "frames"
        frames.mkdir()
        run([
            "docker", "run", "--rm", "--init", "--network", "none",
            "--user", f"{os.getuid()}:{os.getgid()}", "-e", "HOME=/tmp",
            "-e", "PANZER_SHOWCASE_STRIDE=5", "-e", "LIBGL_ALWAYS_SOFTWARE=1",
            "-v", f"{binary}:/panzer:ro", "-v", f"{scratch}:/out", docker_image,
            "xvfb-run", "-a", "-s", "-screen 0 1280x800x24",
            "/panzer", "showcase", item, "/out/frames",
        ])
        count = len(list(frames.glob("*.png")))
        # Older binaries ignore the stride option and still produce all 300 frames.
        if count not in (60, 300):
            raise RuntimeError(f"{item}: expected 60 or 300 frames, got {count}")
        source = ["ffmpeg", "-loglevel", "error", "-y", "-framerate", str(count / 5),
                  "-i", str(frames / "%04d.png")]
        run(source + ["-vf", "fps=4,scale=320:-1,tile=5x4:padding=3:color=0x202630",
                      "-frames:v", "1", str(output / f"{item}.png")])
        run(source + ["-c:v", "libx264", "-preset", "fast", "-crf", "23",
                      "-pix_fmt", "yuv420p", "-movflags", "+faststart", str(output / f"{item}.mp4")])
        return {"item": item, "frames": count, "seconds": 5,
                "sheet": f"{item}.png", "video": f"{item}.mp4"}


def main():
    root = Path(__file__).resolve().parent.parent
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=root / "target/release/panzer")
    parser.add_argument("--output", type=Path, default=root / "target/weapon-review")
    parser.add_argument("--image", default="panzer-headless")
    parser.add_argument("--jobs", type=int, choices=range(1, 5), default=2)
    parser.add_argument("items", nargs="+")
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    for item in args.items:
        if not item.isascii() or not item.isalnum():
            parser.error(f"Invalid Rust item name: {item!r}")
    results = []
    with ThreadPoolExecutor(max_workers=args.jobs) as workers:
        jobs = [workers.submit(record, binary, item, output, args.image) for item in dict.fromkeys(args.items)]
        for job in as_completed(jobs):
            result = job.result()
            results.append(result)
            (output / "manifest.json").write_text(json.dumps(sorted(results, key=lambda r: r['item']), indent=2) + "\n")
            print(f"{result['item']}: {result['frames']} frames checked, sheet + video saved", flush=True)


if __name__ == "__main__":
    main()
