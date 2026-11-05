# Der Launcher (crates/launcher): lädt das Spiel aus den GitHub-Releases und startet es
{
  lib,
  pkgs,
  rustPlatform,
  common,
  pkg-config,
  patchelf,
  makeWrapper,
  xdg-utils,
}:

rustPlatform.buildRustPackage (
  common
  // {
    pname = "panzer-launcher";
    cargoBuildFlags = [ "-p" "panzer-launcher" ];
    cargoTestFlags = [ "-p" "panzer-launcher" ];

    nativeBuildInputs = [ pkg-config patchelf makeWrapper ];
    buildInputs = import ./runtime-libs.nix pkgs;

    postInstall = ''
      install -Dm644 packaging/linux/panzer-launcher.desktop \
        $out/share/applications/panzer-launcher.desktop
      for size in 16 32 48 64 128 256 512; do
        install -Dm644 assets/icon/panzer-$size.png \
          $out/share/icons/hicolor/''${size}x$size/apps/panzer-launcher.png
      done
    '';

    # postFixup, weil patchELF in fixupPhase unbenutzte RUNPATH-Einträge wieder entfernt
    postFixup = ''
      patchelf --add-rpath ${lib.makeLibraryPath (import ./runtime-libs.nix pkgs)} \
        $out/bin/panzer-launcher
      wrapProgram $out/bin/panzer-launcher --suffix PATH : ${lib.makeBinPath [ xdg-utils ]}
    '';

    meta = {
      description = "Launcher für Panzer: lädt das Spiel herunter, hält es aktuell und startet es";
      license = lib.licenses.mit;
      mainProgram = "panzer-launcher";
      platforms = lib.platforms.linux;
    };
  }
)
