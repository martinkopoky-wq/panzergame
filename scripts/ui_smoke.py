"""Native input tests for the game menu, HUD and launcher. Requires X11, Xtst, xwd, ffmpeg and Pillow."""

import ctypes as c
import ctypes.util
import os
import subprocess as sp
import time
import json
from pathlib import Path
import argparse
import signal
import socket
import tempfile

parser = argparse.ArgumentParser(
    description="Native UI smoke test on an isolated, already-running Xvfb display"
)
parser.add_argument("--display", default=":119")
parser.add_argument("--output", type=Path)
args = parser.parse_args()
DISPLAY = args.display
ROOT = Path(__file__).resolve().parent.parent
OUT = args.output or ROOT / "target/ui-redesign/verified"
TEMP = tempfile.TemporaryDirectory(prefix="panzer-ui-smoke-")
OUT.mkdir(parents=True, exist_ok=True)
ENV = dict(
    os.environ,
    DISPLAY=args.display,
    WAYLAND_DISPLAY="",
    WINIT_UNIX_BACKEND="x11",
    LIBGL_ALWAYS_SOFTWARE="1",
    XDG_DATA_HOME=TEMP.name + "/data",
    XDG_CONFIG_HOME=TEMP.name + "/config",
)
X = c.CDLL(ctypes.util.find_library("X11"))
T = c.CDLL(ctypes.util.find_library("Xtst"))
X.XOpenDisplay.argtypes = [c.c_char_p]
X.XOpenDisplay.restype = c.c_void_p
d = X.XOpenDisplay(DISPLAY.encode())
for _ in range(50):
    if d:
        break
    time.sleep(0.1)
    d = X.XOpenDisplay(DISPLAY.encode())
assert d, "Start the isolated Xvfb display first (see docs/UI_DESIGN.md)"
for name, args, ret in [
    ("XDefaultRootWindow", [c.c_void_p], c.c_ulong),
    (
        "XQueryTree",
        [
            c.c_void_p,
            c.c_ulong,
            c.POINTER(c.c_ulong),
            c.POINTER(c.c_ulong),
            c.POINTER(c.POINTER(c.c_ulong)),
            c.POINTER(c.c_uint),
        ],
        c.c_int,
    ),
    ("XFetchName", [c.c_void_p, c.c_ulong, c.POINTER(c.c_char_p)], c.c_int),
    ("XFree", [c.c_void_p], c.c_int),
    ("XSetInputFocus", [c.c_void_p, c.c_ulong, c.c_int, c.c_ulong], c.c_int),
    ("XFlush", [c.c_void_p], c.c_int),
    ("XKeysymToKeycode", [c.c_void_p, c.c_ulong], c.c_uint),
    ("XStringToKeysym", [c.c_char_p], c.c_ulong),
    ("XResizeWindow", [c.c_void_p, c.c_ulong, c.c_uint, c.c_uint], c.c_int),
]:
    f = getattr(X, name)
    f.argtypes = args
    f.restype = ret
T.XTestFakeKeyEvent.argtypes = [c.c_void_p, c.c_uint, c.c_int, c.c_ulong]
T.XTestFakeButtonEvent.argtypes = [c.c_void_p, c.c_uint, c.c_int, c.c_ulong]
T.XTestFakeMotionEvent.argtypes = [c.c_void_p, c.c_int, c.c_int, c.c_int, c.c_ulong]
X.XGetImage.argtypes = [
    c.c_void_p,
    c.c_ulong,
    c.c_int,
    c.c_int,
    c.c_uint,
    c.c_uint,
    c.c_ulong,
    c.c_int,
]
X.XGetImage.restype = c.c_void_p
X.XGetPixel.argtypes = [c.c_void_p, c.c_int, c.c_int]
X.XGetPixel.restype = c.c_ulong
X.XDestroyImage.argtypes = [c.c_void_p]


def pixel(w, x, y):
    img = X.XGetImage(d, w, x, y, 1, 1, c.c_ulong(-1), 2)
    if not img:
        return None
    value = X.XGetPixel(img, 0, 0)
    X.XDestroyImage(img)
    return value


root = X.XDefaultRootWindow(d)


def windows(w=root):
    rt = c.c_ulong()
    p = c.c_ulong()
    children = c.POINTER(c.c_ulong)()
    n = c.c_uint()
    X.XQueryTree(d, w, c.byref(rt), c.byref(p), c.byref(children), c.byref(n))
    out = []
    for child in list(children[: n.value]):
        name = c.c_char_p()
        X.XFetchName(d, child, c.byref(name))
        if name.value:
            if name.value == b"Panzer":
                out.append(child)
            X.XFree(name)
        out += windows(child)
    if children:
        X.XFree(children)
    return out


def key(name, down=True):
    code = X.XKeysymToKeycode(d, X.XStringToKeysym(name.encode()))
    assert code, name
    T.XTestFakeKeyEvent(d, code, down, 0)
    X.XFlush(d)
    time.sleep(0.05)


def tap(name):
    key(name)
    key(name, False)


def click(x, y):
    T.XTestFakeMotionEvent(d, -1, x, y, 0)
    X.XFlush(d)
    time.sleep(0.05)
    T.XTestFakeButtonEvent(d, 1, 1, 0)
    X.XFlush(d)
    time.sleep(0.05)
    T.XTestFakeButtonEvent(d, 1, 0, 0)
    X.XFlush(d)
    time.sleep(0.2)


def shot(w, name):
    raw = str(Path(TEMP.name) / f"{name}.xwd")
    sp.run(
        ["xwd", "-display", DISPLAY, "-id", str(w), "-silent", "-out", raw], check=True
    )
    sp.run(
        [
            "ffmpeg",
            "-v",
            "error",
            "-y",
            "-i",
            raw,
            "-frames:v",
            "1",
            str(OUT / f"{name}.png"),
        ],
        check=True,
    )
    Path(raw).unlink()


def start(binary, *args):
    old = set(windows())
    log = open(OUT / f"{binary}.log", "w")
    p = sp.Popen(
        [str(ROOT / "target/debug" / binary), *args],
        env=ENV,
        stdout=log,
        stderr=log,
        cwd=ROOT,
        start_new_session=True,
    )
    for _ in range(100):
        ws = [w for w in windows() if w not in old]
        if ws:
            time.sleep(0.3)
            for _ in range(200):
                if "Map State: IsViewable" in sp.check_output(
                    ["xwininfo", "-display", DISPLAY, "-id", str(ws[-1])], text=True
                ):
                    if (
                        binary == "panzer"
                        and args
                        and args[0] in ("solo", "local", "host", "join")
                    ) or pixel(ws[-1], 44 if binary == "panzer" else 4, 100) == (
                        0xF7F4EB if binary == "panzer" else 0x1D2324
                    ):
                        break
                time.sleep(0.1)
            else:
                p.terminate()
                p.wait(timeout=5)
                raise RuntimeError("UI did not finish loading")
            X.XSetInputFocus(d, ws[-1], 1, 0)
            X.XFlush(d)
            time.sleep(0.2)
            return p, ws[-1]
        if p.poll() is not None:
            raise RuntimeError(f"{binary} exited {p.returncode}")
        time.sleep(0.1)
    p.terminate()
    raise RuntimeError("no window")


def stop(p):
    # Only this fixture's process group; a launcher may already have exited after spawning its game.
    try:
        os.killpg(p.pid, signal.SIGTERM)
    except ProcessLookupError:
        pass
    try:
        p.wait(timeout=5)
    except sp.TimeoutExpired:
        p.kill()
        p.wait()
    time.sleep(0.1)


def wait_until(condition, description, seconds=8):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        if condition():
            return
        time.sleep(0.1)
    raise AssertionError(description)


def type_text(text):
    symbols = {".": "period", ":": "colon", "-": "minus", " ": "space"}
    for char in text:
        tap(symbols.get(char, char))


def assert_validation(w, name):
    from PIL import Image

    shot(w, name)
    image = Image.open(OUT / f"{name}.png").convert("RGB").crop((64, 496, 368, 520))
    red = sum(
        r > 120 and g < 110 and b < 100 for r, g, b in zip(*[iter(image.tobytes())] * 3)
    )
    assert red > 20, f"{name}: expected a visible validation error"


def main():
    checks = []
    config = Path(ENV["XDG_CONFIG_HOME"]) / "panzer/config.txt"
    config.parent.mkdir(parents=True, exist_ok=True)
    config.write_text("name=\nip=\nbots=3\nsfx=0\nmusic=0\n")
    p, w = start("panzer")
    try:
        shot(w, "menu")
        click(610, 152)
        click(210, 556)
        assert_validation(w, "name-validation")
        checks.append("online name validation")
        click(150, 348)
        type_text("martin")
        click(352, 400)
        click(352, 450)
        click(148, 152)
        tap("Return")
        time.sleep(1.2)
        data = config.read_text()
        assert "name=martin" in data and "bots=4" in data and "rule=keine" in data, data
        shot(w, "solo-five")
        checks.append("solo start and saved name/bots/rule")
        tap("Escape")
        time.sleep(0.3)
        wait_until(lambda: pixel(w, 44, 100) == 0xF7F4EB, "return to menu")
        click(382, 152)
        click(216, 344)
        click(210, 556)
        time.sleep(1)
        shot(w, "local-three")
        checks.append("three players on one keyboard")
        tap("Escape")
        time.sleep(0.3)
        wait_until(lambda: pixel(w, 44, 100) == 0xF7F4EB, "local return to menu")
        click(846, 152)
        click(210, 556)
        assert_validation(w, "ip-validation")
        checks.append("empty join address validation")
        # Hosting is tested only when it will not collide with a real game on the owner's PC.
        probe = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        try:
            probe.bind(("0.0.0.0", 5555))
            available = True
        except OSError:
            available = False
        finally:
            probe.close()
        if available:
            click(610, 152)
            click(210, 556)
            time.sleep(1.5)
            key("Tab")
            shot(w, "scoreboard")
            key("Tab", False)
            tap("t")
            shot(w, "chat")
            tap("Escape")
            tap("Escape")
            time.sleep(0.3)
            wait_until(lambda: pixel(w, 44, 100) == 0xF7F4EB, "host return to menu")
            checks.append("hosting, player list, chat and return")
        else:
            checks.append("hosting skipped: UDP 5555 already in use")
    finally:
        stop(p)
    p, w = start("panzer", "solo", "7")
    try:
        time.sleep(1.5)
        shot(w, "hud-eight")
        checks.append("eight-player HUD rendered")
    finally:
        stop(p)
    p, w = start("panzer-launcher", "--local")
    try:
        shot(w, "launcher")
        click(88, 174)
        time.sleep(0.6)
        shot(w, "weapons")
        time.sleep(0.5)
        shot(w, "weapons-animation")
        from PIL import Image, ImageChops

        first = (
            Image.open(OUT / "weapons.png").convert("RGB").crop((216, 186, 562, 421))
        )
        later = (
            Image.open(OUT / "weapons-animation.png")
            .convert("RGB")
            .crop((216, 186, 562, 421))
        )
        assert ImageChops.difference(
            first, later
        ).getbbox(), "weapon preview must animate"
        click(448, 112)
        time.sleep(0.3)
        shot(w, "effects-filter")
        checks.append("weapon browser, animated frames and effects filter")
        click(88, 230)
        time.sleep(0.3)
        shot(w, "news")
        checks.append("release notes navigation")
        click(88, 118)
        time.sleep(0.2)
        X.XResizeWindow(d, w, 860, 540)
        X.XFlush(d)
        time.sleep(0.4)
        shot(w, "launcher-minimum")
        click(88, 174)
        time.sleep(0.3)
        shot(w, "weapons-minimum")
        checks.append("860x540 launcher")
        click(88, 118)
        X.XResizeWindow(d, w, 1040, 640)
        X.XFlush(d)
        time.sleep(0.3)
        old = set(windows())
        click(910, 566)
        wait_until(lambda: p.poll() is not None, "launcher close-on-play")
        wait_until(lambda: bool(set(windows()) - old), "local game launched")
        child = list(set(windows()) - old)[-1]
        time.sleep(1)
        wait_until(lambda: pixel(child, 44, 100) == 0xF7F4EB, "local game menu ready")
        shot(child, "launched-game")
        checks.append("test launcher starts current local game")
    finally:
        stop(p)
    saved = (
        "\n".join(
            line
            for line in config.read_text().splitlines()
            if not line.startswith("token=")
        )
        + "\n"
    )
    report = {
        "result": "passed",
        "platform": "Linux / isolated Xvfb",
        "checks": checks,
        "artifacts": str(OUT),
        "config": saved,
    }
    (OUT / "review.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2), flush=True)


if __name__ == "__main__":
    main()
