{
  description = "Panzer – Labyrinth-Panzerschlacht: Spiel, Launcher, dedizierter Server und Docker-Image";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    { self, nixpkgs, rust-overlay }:
    let
      inherit (nixpkgs) lib;
      systems = [ "x86_64-linux" "aarch64-linux" ];
      forAllSystems =
        f:
        lib.genAttrs systems (
          system:
          f (import nixpkgs {
            inherit system;
            overlays = [ rust-overlay.overlays.default ];
          })
        );
    in
    {
      packages = forAllSystems (
        pkgs:
        let
          # Fest über flake.lock, also für alle dieselbe Rust-Version
          toolchain = pkgs.rust-bin.stable.latest.minimal;
          rustPlatform = pkgs.makeRustPlatform {
            cargo = toolchain;
            rustc = toolchain;
          };

          # Der Server soll ohne Betriebssystem laufen: statisch gegen musl gelinkt
          staticToolchain = toolchain.override {
            targets = [ pkgs.pkgsStatic.stdenv.hostPlatform.rust.rustcTarget ];
          };
          staticRustPlatform = pkgs.pkgsStatic.makeRustPlatform {
            cargo = staticToolchain;
            rustc = staticToolchain;
          };

          common = {
            version = (lib.importTOML ./Cargo.toml).workspace.package.version;
            # Nur was den Build beeinflusst, sonst baut jede README-Änderung alles neu
            src = lib.fileset.toSource {
              root = ./.;
              fileset = lib.fileset.unions [
                ./Cargo.toml
                ./Cargo.lock
                ./crates
                ./assets
                ./packaging/linux/panzer-launcher.desktop
              ];
            };
            cargoLock.lockFile = ./Cargo.lock;
          };
        in
        rec {
          panzer = pkgs.callPackage ./nix/panzer.nix { inherit common rustPlatform; };
          panzer-launcher = pkgs.callPackage ./nix/panzer-launcher.nix { inherit common rustPlatform; };
          panzer-server = pkgs.pkgsStatic.callPackage ./nix/panzer-server.nix {
            inherit common;
            rustPlatform = staticRustPlatform;
          };
          panzer-server-image = pkgs.callPackage ./nix/panzer-server-image.nix { inherit panzer-server; };
          default = panzer;
        }
      );

      checks = forAllSystems (pkgs: {
        inherit (self.packages.${pkgs.stdenv.hostPlatform.system})
          panzer
          panzer-launcher
          panzer-server
          panzer-server-image
          ;
      });

      devShells = forAllSystems (pkgs: {
        default = pkgs.mkShell {
          inputsFrom = [
            self.packages.${pkgs.stdenv.hostPlatform.system}.panzer
            self.packages.${pkgs.stdenv.hostPlatform.system}.panzer-launcher
          ];
          packages = [
            (pkgs.rust-bin.stable.latest.default.override {
              extensions = [ "rust-src" "rust-analyzer" ];
            })
          ];
          LD_LIBRARY_PATH = lib.makeLibraryPath (import ./nix/runtime-libs.nix pkgs);
        };
      });
    };
}
