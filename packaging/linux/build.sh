#!/usr/bin/env bash
# Baut alle Linux-Downloads nach dist/:
#   Panzer-linux-x86_64.AppImage, panzer-launcher_amd64.deb   Launcher
#   panzer-game-linux-x86_64.zip                              Spielpaket, das der Launcher lädt
#   panzer-server-linux-x86_64.tar.gz                         dedizierter Server
# Die Dateinamen enthalten keine Version, sonst brechen die Links auf der Website.
set -euo pipefail
cd "$(dirname "$0")/../.."

VERSION=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n1)
DIST=$PWD/dist
WORK=$PWD/target/packaging/linux
TOOLS=$PWD/target/packaging/tools

# Werkzeuge mit fester Version und Prüfsumme: landen direkt im Release
APPIMAGETOOL_URL=https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-x86_64.AppImage
APPIMAGETOOL_SHA=ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0
RUNTIME_URL=https://github.com/AppImage/type2-runtime/releases/download/20251108/runtime-x86_64
RUNTIME_SHA=2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d

fetch() { # url sha256 datei
    if [ ! -f "$3" ] || ! echo "$2  $3" | sha256sum -c --quiet - >/dev/null 2>&1; then
        curl -fsSL --proto '=https' -o "$3" "$1"
        echo "$2  $3" | sha256sum -c --quiet -
    fi
}

cargo build --release --locked -p panzer -p panzer-launcher -p panzer-server

rm -rf "$WORK"
mkdir -p "$DIST" "$WORK" "$TOOLS"

# Spielpaket und Server
rm -f "$DIST/panzer-game-linux-x86_64.zip"
zip -q -j "$DIST/panzer-game-linux-x86_64.zip" target/release/panzer
tar -czf "$DIST/panzer-server-linux-x86_64.tar.gz" -C target/release panzer-server

# .deb
DEB=$WORK/deb
install -Dm755 target/release/panzer-launcher "$DEB/usr/bin/panzer-launcher"
install -Dm644 packaging/linux/panzer-launcher.desktop "$DEB/usr/share/applications/panzer-launcher.desktop"
for size in 16 32 48 64 128 256 512; do
    install -Dm644 "assets/icon/panzer-$size.png" "$DEB/usr/share/icons/hicolor/${size}x${size}/apps/panzer-launcher.png"
done
mkdir -p "$DEB/DEBIAN"
cat >"$DEB/DEBIAN/control" <<EOF
Package: panzer-launcher
Version: $VERSION
Architecture: amd64
Maintainer: martinkopoky-wq <martinkopoky-wq@users.noreply.github.com>
Section: games
Priority: optional
Homepage: https://martinkopoky-wq.github.io/panzergame/
Depends: libc6, libgcc-s1, libasound2 | libasound2t64, libgl1, libx11-6, libxi6, libxkbcommon0
Recommends: libwayland-client0, libwayland-egl1, libxcursor1
Description: Launcher für Panzer, eine Labyrinth-Panzerschlacht
 Lädt das Spiel herunter, hält es aktuell und startet es.
 Bis zu 8 Spieler über LAN und Internet, dazu Bots und 93 Power-ups.
EOF
dpkg-deb -Zxz --root-owner-group --build "$DEB" "$DIST/panzer-launcher_amd64.deb" >/dev/null

# AppImage
TOOL=$TOOLS/appimagetool-x86_64.AppImage
RUNTIME=$TOOLS/runtime-x86_64
fetch "$APPIMAGETOOL_URL" "$APPIMAGETOOL_SHA" "$TOOL"
fetch "$RUNTIME_URL" "$RUNTIME_SHA" "$RUNTIME"
chmod +x "$TOOL"

APPDIR=$WORK/Panzer.AppDir
install -Dm755 target/release/panzer-launcher "$APPDIR/usr/bin/panzer-launcher"
install -Dm644 packaging/linux/panzer-launcher.desktop "$APPDIR/panzer-launcher.desktop"
install -Dm644 assets/icon/panzer-256.png "$APPDIR/panzer-launcher.png"
ln -s usr/bin/panzer-launcher "$APPDIR/AppRun"
# Im Container gibt es kein FUSE: das Werkzeug entpackt sich dann selbst
ARCH=x86_64 APPIMAGE_EXTRACT_AND_RUN=1 "$TOOL" --no-appstream --runtime-file "$RUNTIME" \
    "$APPDIR" "$DIST/Panzer-linux-x86_64.AppImage" >/dev/null 2>&1

ls -l "$DIST"
