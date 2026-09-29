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
        coverageRunner = pkgs.writeShellApplication {
          name = "bombay-behavior-coverage";
          runtimeInputs = [ rustToolchain pkgs.cargo-llvm-cov ];
          text = ''
            if [[ ! -f Cargo.toml || ! -d crates/actors ]]; then
              echo "run this command from the bombay-behavior repository root" >&2
              exit 2
            fi
            exec cargo llvm-cov --workspace --lib --tests --locked "$@"
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
            ./scripts/check_rustdoc_imports.py
            ./scripts/check_rustdoc_error_codes.py
            (pkgs.lib.fileset.maybeMissing ./mutants-baseline.json)
            ./mutants/actors
          ];
        };
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
              python3 scripts/check_rustdoc_imports.py
              python3 scripts/check_rustdoc_error_codes.py
              python3 -m unittest scripts/test_check_assertion_effects.py
              python3 scripts/check_assertion_effects.py
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
          coverage = coverageRunner;

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

          # On-demand actor-family slices. Each reviewed floor is committed,
          # so a later rerun cannot silently turn a caught mutant unviable.
          mutants-actors = craneLib.mkCargoDerivation (commonArgs // {
            inherit cargoArtifacts;
            pnameSuffix = "-actors-mutants";
            nativeBuildInputs = [ pkgs.cargo-mutants pkgs.cargo-nextest ];
            buildPhaseCargoCommand = ''
              set -euo pipefail
              mkdir -p "$out"
              run_actor_campaign() {
                campaign="$1"
                file="$2"
                filter="$3"
                PROPTEST_CASES=64 cargo mutants \
                  --package bombay-behavior-actors \
                  --test-package bombay-behavior-actors \
                  --test-package bombay-behavior-testkit \
                  --test-tool nextest --no-shuffle --colors never \
                  --minimum-test-timeout 180 \
                  -f "$file" -F "$filter" \
                  --output "$out/$campaign" -- --profile mutants || true
                cargo run --release -p behavior-mutants-gate -- \
                  check "$out/$campaign/mutants.out" \
                  "$PWD/mutants/actors/$campaign.json" \
                  | tee "$out/$campaign/mutants-gate-report.txt"
              }
              run_actor_campaign health \
                crates/actors/src/operations/health.rs \
                'Health<A, K, Route>::commit'
              run_actor_campaign work_queue \
                crates/actors/src/routing/work_queue.rs \
                '::submit|::announce'
              run_actor_campaign pub_sub_membership \
                crates/actors/src/discovery/pub_sub.rs \
                'PubSub<A, K, P, Route>::subscribe|PubSub<A, K, P, Route>::unsubscribe'
              run_actor_campaign pub_sub_publish \
                crates/actors/src/discovery/pub_sub.rs \
                '<impl Behavior for PubSub<A, K, P, Route>>::transition'
            '';
            doInstallCargoArtifacts = false;
            doCheck = false;
          });
        };

        devShells.default = craneLib.devShell {
          checks = self.checks.${system};
          packages = with pkgs; [ cargo-audit cargo-deny cargo-llvm-cov cargo-mutants cargo-nextest taplo ];
        };
        devShells.fuzz = pkgs.mkShell {
          packages = [ fuzzToolchain pkgs.cargo-fuzz ];
        };
      });
}
