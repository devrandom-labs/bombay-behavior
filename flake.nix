{
  description = "Composable, statically typed actor behavior algebra";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    utils.url = "github:numtide/flake-utils";
    crane.url = "github:ipetkov/crane";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
    };
  };

  outputs = { self, nixpkgs, utils, crane, fenix, advisory-db, ... }:
    utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        rustToolchain = fenix.packages.${system}.fromToolchainFile {
          file = ./rust-toolchain.toml;
          sha256 = "sha256-gh/xTkxKHL4eiRXzWv8KP7vfjSk61Iq48x47BEDFgfk=";
        };
        craneLib = (crane.mkLib pkgs).overrideToolchain (_: rustToolchain);
        src = craneLib.cleanCargoSource ./.;
        commonArgs = {
          inherit src;
          strictDeps = true;
        };
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;
      in {
        checks = {
          bombay-behavior = craneLib.buildPackage (commonArgs // { inherit cargoArtifacts; });
          bombay-behavior-nextest = craneLib.cargoNextest (commonArgs // {
            inherit cargoArtifacts;
            cargoNextestExtraArgs = "--workspace";
          });
          bombay-behavior-doc = craneLib.cargoDoc (commonArgs // {
            inherit cargoArtifacts;
            cargoDocExtraArgs = "--workspace --no-deps";
          });
          bombay-behavior-fmt = craneLib.cargoFmt { inherit src; };
          bombay-behavior-toml-fmt = craneLib.taploFmt {
            src = pkgs.lib.sources.sourceFilesBySuffices src [ ".toml" ];
          };
          bombay-behavior-audit = craneLib.cargoAudit { inherit src advisory-db; };
          bombay-behavior-deny = craneLib.cargoDeny { inherit src; };
        };

        packages.default = craneLib.buildPackage (commonArgs // { inherit cargoArtifacts; });

        devShells.default = craneLib.devShell {
          checks = self.checks.${system};
          packages = with pkgs; [ cargo-audit cargo-deny cargo-nextest taplo ];
        };
      });
}
