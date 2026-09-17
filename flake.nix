{
  description = "Presence: Autonomous Dasein Cognitive Agent System & Dynamic Organ Ecosystem";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        manifest = (pkgs.lib.importTOML ./Cargo.toml).package;
      in rec {
        packages = {
          presence = pkgs.rustPlatform.buildRustPackage {
            pname = manifest.name;
            version = manifest.version;
            src = ./.;
            cargoLock = {
              lockFile = ./Cargo.lock;
            };

            nativeBuildInputs = with pkgs; [ pkg-config ];
            buildInputs = with pkgs; [ openssl ];

            postInstall = ''
              mkdir -p $out/share/presence/seed
              if [ -d ./seed ]; then
                cp -r ./seed/* $out/share/presence/seed/
              fi
            '';

            meta = with pkgs.lib; {
              description = "Conscious autonomous agent triad (Cortex, Stem, Cord)";
              homepage = "https://github.com/hrkcz001/presence";
              license = licenses.mit;
              mainProgram = "presence";
            };
          };

          organ-vox = pkgs.stdenv.mkDerivation {
            pname = "presence-organ-vox";
            version = "1.0.0";
            src = ./organs/vox;

            installPhase = ''
              mkdir -p $out/share/presence/organs/vox
              cp -r ./* $out/share/presence/organs/vox/
            '';

            meta = with pkgs.lib; {
              description = "Pure-Rust native voice organ for Presence";
              homepage = "https://github.com/hrkcz001/presence";
              license = licenses.mit;
            };
          };

          organ-git = pkgs.stdenv.mkDerivation {
            pname = "presence-organ-git";
            version = "1.0.0";
            src = ./organs/git;

            installPhase = ''
              mkdir -p $out/share/presence/organs/git
              cp -r ./* $out/share/presence/organs/git/
            '';

            meta = with pkgs.lib; {
              description = "Git interaction organ for Presence";
              homepage = "https://github.com/hrkcz001/presence";
              license = licenses.mit;
            };
          };

          organ-io = pkgs.stdenv.mkDerivation {
            pname = "presence-organ-io";
            version = "1.0.0";
            src = ./organs/io;

            installPhase = ''
              mkdir -p $out/share/presence/organs/io
              cp -r ./* $out/share/presence/organs/io/
            '';

            meta = with pkgs.lib; {
              description = "Filesystem interaction and safe I/O organ for Presence";
              homepage = "https://github.com/hrkcz001/presence";
              license = licenses.mit;
            };
          };

          default = packages.presence;
        };

        apps = {
          default = flake-utils.lib.mkApp {
            drv = packages.presence;
          };
        };

        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            cargo
            rustc
            clippy
            rustfmt
            nodejs_22
            bun
          ];
        };
      }
    ) // {
      overlays.default = final: prev: {
        presence = self.packages.${final.system}.presence;
        presence-organ-vox = self.packages.${final.system}.organ-vox;
        presence-organ-git = self.packages.${final.system}.organ-git;
        presence-organ-io = self.packages.${final.system}.organ-io;
      };
    };
}