# Der dedizierte Server (crates/server), hier statisch gegen musl gelinkt
{ lib, rustPlatform, common }:

rustPlatform.buildRustPackage (
  common
  // {
    pname = "panzer-server";
    cargoBuildFlags = [ "-p" "panzer-server" ];
    # panzer-core hat kein eigenes Programm, seine Tests laufen deshalb hier mit
    cargoTestFlags = [ "-p" "panzer-server" "-p" "panzer-core" ];

    meta = {
      description = "Dedizierter Server für Panzer: autoritativ, mit Anti-Cheat, Chat, Kick/Bann";
      license = lib.licenses.mit;
      mainProgram = "panzer-server";
      platforms = lib.platforms.linux;
    };
  }
)
