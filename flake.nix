{
  description = "Composable, statically typed actor behavior primitives";

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
        fuzzToolchain = fenix.packages.${system}.latest.withComponents [
          "cargo"
          "rustc"
          "rust-src"
          "rust-std"
          "llvm-tools-preview"
        ];
        fuzzRunner = pkgs.writeShellApplication {
          name = "bombay-behavior-fuzz";
          runtimeInputs = [ fuzzToolchain pkgs.cargo-fuzz ];
          text = ''
            fuzz_manifest="crates/behavior-testkit/fuzz/Cargo.toml"
            if [[ ! -f "$fuzz_manifest" ]]; then
              echo "run this command from the bombay-behavior repository root" >&2
              exit 2
            fi
            cd "$(dirname "$fuzz_manifest")"
            exec cargo fuzz "$@"
          '';
        };
        craneLib = (crane.mkLib pkgs).overrideToolchain (_: rustToolchain);
        src = pkgs.lib.fileset.toSource {
          root = ./.;
          fileset = pkgs.lib.fileset.unions [
            (craneLib.fileset.commonCargoSources ./.)
            ./.cargo/mutants.toml
            ./.config/nextest.toml
            ./LICENSE-APACHE
            ./LICENSE-MIT
            ./README.md
            ./.github/pages-index.html
            ./crates/actors/LICENSE-APACHE
            ./crates/actors/LICENSE-MIT
            ./crates/actors/README.md
            ./crates/behavior/LICENSE-APACHE
            ./crates/behavior/LICENSE-MIT
            ./crates/behavior/README.md
            ./crates/behavior-macros/LICENSE-APACHE
            ./crates/behavior-macros/LICENSE-MIT
            ./crates/behavior-macros/README.md
            ./docs
            ./scripts/check_published_docs.py
            ./scripts/test_check_published_docs.py
            ./scripts/check_published_packages.sh
            (pkgs.lib.fileset.maybeMissing ./mutants-baseline.json)
          ];
        };
        commonArgs = {
          inherit src;
          strictDeps = true;
        };
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        # Ceiling-split form of the `mutants` campaign: one derivation per
        # shard, each running the full campaign argument set with its own
        # baseline run. All shards share the same arguments and denominator,
        # and use round-robin distribution so shards finish at similar times.
        # The CI aggregate job merges the shard results and runs the strict
        # `behavior-mutants-gate` verdict over the union; the unsharded
        # `mutants` package remains the local full-campaign entry point.
        mkMutantsShard = k:
          craneLib.mkCargoDerivation (commonArgs // {
            inherit cargoArtifacts;
            pnameSuffix = "-mutants-shard${toString k}";
            nativeBuildInputs = [ pkgs.cargo-mutants pkgs.cargo-nextest ];
            buildPhaseCargoCommand = ''
              set -o pipefail
              PROPTEST_CASES=64 cargo mutants \
                --package bombay-behavior \
                --test-package bombay-behavior \
                --test-package bombay-behavior-testkit \
                --test-tool nextest --no-shuffle --colors never \
                --minimum-test-timeout 180 \
                --sharding round-robin \
                --shard ${toString k}/8 \
                --output "$out" -- --profile mutants || true
            '';
            doInstallCargoArtifacts = false;
            doCheck = false;
          });
      in {
        checks = {
          bombay-behavior = craneLib.buildPackage (commonArgs // { inherit cargoArtifacts; });
          bombay-behavior-nextest = craneLib.cargoNextest (commonArgs // {
            inherit cargoArtifacts;
            cargoNextestExtraArgs = "--workspace";
          });
          bombay-behavior-clippy = craneLib.cargoClippy (commonArgs // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--workspace --all-targets";
          });
          bombay-behavior-doc = craneLib.mkCargoDerivation (commonArgs // {
            inherit cargoArtifacts;
            pnameSuffix = "-doc";
            nativeBuildInputs = [ pkgs.mdbook pkgs.python3 ];
            buildPhaseCargoCommand = ''
              export RUSTDOCFLAGS="-D warnings"
              cargo doc -p bombay-behavior -p bombay-behavior-actors --no-deps
              mdbook build docs
              cp .github/pages-index.html target/doc/index.html
              python3 -m unittest scripts/test_check_published_docs.py
              python3 scripts/check_published_docs.py target/doc
            '';
            doInstallCargoArtifacts = false;
            doCheck = false;
          });
          bombay-behavior-doctest = craneLib.mkCargoDerivation (commonArgs // {
            inherit cargoArtifacts;
            pnameSuffix = "-doctest";
            buildPhaseCargoCommand = "cargo test --workspace --doc";
            doInstallCargoArtifacts = false;
            doCheck = false;
          });
          bombay-behavior-fmt = craneLib.cargoFmt { inherit src; };
          bombay-behavior-toml-fmt = craneLib.taploFmt {
            src = pkgs.lib.sources.sourceFilesBySuffices src [ ".toml" ];
          };
          bombay-behavior-audit = craneLib.cargoAudit { inherit src advisory-db; };
          bombay-behavior-deny = craneLib.cargoDeny { inherit src; };
          bombay-behavior-package = craneLib.mkCargoDerivation (commonArgs // {
            inherit cargoArtifacts;
            pnameSuffix = "-package";
            buildPhaseCargoCommand =
              "BOMBAY_PACKAGE_MODE=list bash scripts/check_published_packages.sh";
            doInstallCargoArtifacts = false;
            doCheck = false;
          });
        };

        packages = rec {
          default = craneLib.buildPackage (commonArgs // { inherit cargoArtifacts; });
          fuzz = fuzzRunner;

          # Expensive on-demand lane. The gate rejects survivors, timeouts,
          # incomplete runs, and per-function viability regressions. Keep it
          # outside `nix flake check`.
          mutants = craneLib.mkCargoDerivation (commonArgs // {
            inherit cargoArtifacts;
            pnameSuffix = "-mutants";
            nativeBuildInputs = [ pkgs.cargo-mutants pkgs.cargo-nextest ];
            buildPhaseCargoCommand = ''
              set -o pipefail
              PROPTEST_CASES=64 cargo mutants \
                --package bombay-behavior \
                --test-package bombay-behavior \
                --test-package bombay-behavior-testkit \
                --test-tool nextest --no-shuffle --colors never \
                --minimum-test-timeout 180 \
                --output "$out" -- --profile mutants || true
              cargo run --release -p behavior-mutants-gate -- \
                check "$out/mutants.out" "$PWD/mutants-baseline.json" \
                | tee "$out/mutants-gate-report.txt"
            '';
            doInstallCargoArtifacts = false;
            doCheck = false;
          });

          mutants-shard0 = mkMutantsShard 0;
          mutants-shard1 = mkMutantsShard 1;
          mutants-shard2 = mkMutantsShard 2;
          mutants-shard3 = mkMutantsShard 3;
          mutants-shard4 = mkMutantsShard 4;
          mutants-shard5 = mkMutantsShard 5;
          mutants-shard6 = mkMutantsShard 6;
          mutants-shard7 = mkMutantsShard 7;

          # Seeder for the reviewed per-function viability ratchet. Copy the
          # emitted baseline into the repository only after reviewing every
          # survivor, timeout, and zero-viable function.
          mutants-sweep = craneLib.mkCargoDerivation (commonArgs // {
            inherit cargoArtifacts;
            pnameSuffix = "-mutants-sweep";
            nativeBuildInputs = [ pkgs.cargo-mutants pkgs.cargo-nextest ];
            buildPhaseCargoCommand = ''
              PROPTEST_CASES=64 cargo mutants \
                --package bombay-behavior \
                --test-package bombay-behavior \
                --test-package bombay-behavior-testkit \
                --test-tool nextest --no-shuffle --colors never \
                --minimum-test-timeout 180 \
                --output "$out" -- --profile mutants || true
              cargo run --release -p behavior-mutants-gate -- \
                emit-baseline "$out/mutants.out" > "$out/mutants-baseline.json"
              cp -f "$out/mutants.out/missed.txt" "$out/missed.txt" 2>/dev/null || true
              cp -f "$out/mutants.out/timeout.txt" "$out/timeout.txt" 2>/dev/null || true
            '';
            doInstallCargoArtifacts = false;
            doCheck = false;
          });
        };

        devShells.default = craneLib.devShell {
          checks = self.checks.${system};
          packages = with pkgs; [ cargo-audit cargo-deny cargo-mutants cargo-nextest taplo ];
        };
        devShells.fuzz = pkgs.mkShell {
          packages = [ fuzzToolchain pkgs.cargo-fuzz ];
        };
      });
}
