# Das Spiel (crates/client)
{
  lib,
  pkgs,
  rustPlatform,
  common,
  pkg-config,
  patchelf,
  makeDesktopItem,
  copyDesktopItems,
  alsa-lib,
}:

rustPlatform.buildRustPackage (
  common
  // {
    pname = "panzer";
    cargoBuildFlags = [ "-p" "panzer" ];
    cargoTestFlags = [ "-p" "panzer" ];

    nativeBuildInputs = [ pkg-config patchelf copyDesktopItems ];
    buildInputs = [ alsa-lib ] ++ import ./runtime-libs.nix pkgs;

    desktopItems = [
      (makeDesktopItem {
        name = "panzer";
        desktopName = "Panzer";
        comment = "Labyrinth-Panzerschlacht für bis zu 8 Spieler";
        exec = "panzer";
        icon = "panzer";
        categories = [ "Game" "ActionGame" ];
      })
    ];

    postInstall = ''
      for size in 16 32 48 64 128 256 512; do
        install -Dm644 assets/icon/panzer-$size.png \
          $out/share/icons/hicolor/''${size}x$size/apps/panzer.png
      done
    '';

    # postFixup, weil patchELF in fixupPhase unbenutzte RUNPATH-Einträge wieder entfernt
    postFixup = ''
      patchelf --add-rpath ${lib.makeLibraryPath (import ./runtime-libs.nix pkgs)} $out/bin/panzer
    '';

    meta = {
      description = "Labyrinth-Panzerschlacht mit Grafik und Ton";
      license = lib.licenses.mit;
      mainProgram = "panzer";
      platforms = lib.platforms.linux;
    };
  }
)
