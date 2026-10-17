"""Compile the README consumer against the contents of packaged crate archives."""

import json
import os
import pathlib
import re
import subprocess
import tarfile
import tempfile
import tomllib


ROOT = pathlib.Path(__file__).resolve().parent.parent
PACKAGES = (
    "bombay-behavior-macros",
    "bombay-behavior",
    "bombay-behavior-actors",
)


def check_compatible_macro(metadata: dict, macros: pathlib.Path, line: str) -> None:
    """Exercise a compatible candidate, or retain the published compatible macro."""
    selected = [
        package for package in metadata["packages"]
        if package["name"] == "bombay-behavior-macros"
    ]
    if len(selected) != 1:
        raise SystemExit("published consumer has ambiguous macro owners")
    selected = selected[0]
    if not selected["version"].startswith(line + "."):
        raise SystemExit("published consumer escaped its compatible macro version line")
    candidate = tomllib.loads((macros / "Cargo.toml").read_text())["package"]
    selected_manifest = pathlib.Path(selected["manifest_path"]).resolve()
    candidate_manifest = (macros / "Cargo.toml").resolve()
    if candidate["version"].startswith(line + "."):
        if selected_manifest != candidate_manifest or selected["source"] is not None:
            raise SystemExit("published consumer did not exercise the compatible candidate")
    elif selected_manifest == candidate_manifest or selected["source"] != (
        "registry+https://github.com/rust-lang/crates.io-index"
    ):
        raise SystemExit("published consumer selected the incompatible candidate")
    print(
        f"Compatible{line} consumer resolves Macros{selected['version']} "
        f"from {selected['source'] or selected_manifest}",
        flush=True,
    )


def check_published_worker(temporary: pathlib.Path, macros: pathlib.Path) -> None:
    """An old public worker must survive a compatible macro update."""
    consumer = temporary / "published-worker"
    environment = {**os.environ, "CARGO_TARGET_DIR": str(consumer / "target")}
    (consumer / "src").mkdir(parents=True)
    (consumer / "src/main.rs").write_text(
        (ROOT / "crates/behavior-macros/tests/fixtures/clerk.rs").read_text()
    )
    manifest = consumer / "Cargo.toml"
    manifest.write_text(
        '[package]\nname = "published-pool-worker"\nversion = "0.0.0"\n'
        'edition = "2024"\n\n[dependencies]\n'
        'behavior = { package = "bombay-behavior", version = "=0.26.1" }\n'
        'behavior_actors = { package = "bombay-behavior-actors", version = "=0.26.1" }\n'
        '\n[patch.crates-io]\n'
        f'bombay-behavior-macros = {{ path = "{macros}" }}\n'
    )
    metadata = json.loads(
        subprocess.check_output(
            ["cargo", "metadata", "--format-version", "1", "--manifest-path", str(manifest)],
            cwd=temporary,
            env=environment,
        )
    )
    for name in ("bombay-behavior", "bombay-behavior-actors"):
        owners = [package for package in metadata["packages"] if package["name"] == name]
        if len(owners) != 1:
            raise SystemExit(f"historical worker has ambiguous {name} owners")
        package = owners[0]
        if package["version"] != "0.26.1" or package["source"] != (
            "registry+https://github.com/rust-lang/crates.io-index"
        ):
            raise SystemExit(f"historical worker did not select published {name}0.26.1")
    check_compatible_macro(metadata, macros, "0.14")
    subprocess.run(
        ["cargo", "check", "--locked", "--manifest-path", str(manifest)],
        cwd=temporary,
        env=environment,
        check=True,
    )


def check_published_product(temporary: pathlib.Path, macros: pathlib.Path) -> None:
    """Preserve an existing inferred send with a handwritten insertion witness."""
    consumer = temporary / "published-product"
    environment = {**os.environ, "CARGO_TARGET_DIR": str(consumer / "target")}
    (consumer / "src").mkdir(parents=True)
    (consumer / "src/main.rs").write_text(
        (ROOT / "crates/behavior-macros/tests/fixtures/notices.rs").read_text()
    )
    manifest = consumer / "Cargo.toml"
    manifest.write_text(
        '[package]\nname = "published-send-product"\nversion = "0.0.0"\n'
        'edition = "2024"\n\n[dependencies]\n'
        'behavior = { package = "bombay-behavior", version = "=0.28.0" }\n'
        'behavior-macros = { package = "bombay-behavior-macros", version = "0.15" }\n'
        '\n[patch.crates-io]\n'
        f'bombay-behavior-macros = {{ path = "{macros}" }}\n'
    )
    metadata = json.loads(
        subprocess.check_output(
            ["cargo", "metadata", "--format-version", "1", "--manifest-path", str(manifest)],
            cwd=temporary,
            env=environment,
        )
    )
    owners = [package for package in metadata["packages"]
              if package["name"] == "bombay-behavior"]
    if len(owners) != 1 or owners[0]["version"] != "0.28.0" or owners[0]["source"] != (
        "registry+https://github.com/rust-lang/crates.io-index"
    ):
        raise SystemExit("historical product did not select published Behavior0.28.0")
    check_compatible_macro(metadata, macros, "0.15")
    subprocess.run(
        ["cargo", "run", "--locked", "--manifest-path", str(manifest)],
        cwd=temporary,
        env=environment,
        check=True,
    )


def main() -> None:
    metadata = json.loads(
        subprocess.check_output(
            ["cargo", "metadata", "--format-version", "1", "--no-deps"], cwd=ROOT
        )
    )
    versions = {
        package["name"]: package["version"] for package in metadata["packages"]
    }
    archive_dir = pathlib.Path(metadata["target_directory"]) / "package"
    readme = (ROOT / "README.md").read_text()
    dependency_block = re.search(r"```toml\n(\[dependencies\][\s\S]*?)\n```", readme)
    if dependency_block is None:
        raise SystemExit("README has no installation dependency block")
    declared = tomllib.loads(dependency_block.group(1))["dependencies"]
    if set(declared) != set(PACKAGES[1:]):
        raise SystemExit("README dependencies differ from published consumer packages")
    for package in PACKAGES[1:]:
        if not versions[package].startswith(declared[package] + "."):
            raise SystemExit(f"README version does not select {package} archive")

    with tempfile.TemporaryDirectory(prefix="bombay-published-consumer-") as directory:
        temporary = pathlib.Path(directory)
        extracted = {}
        for package in PACKAGES:
            archive = archive_dir / f"{package}-{versions[package]}.crate"
            if not archive.is_file():
                raise SystemExit(f"missing packaged archive: {archive}")
            with tarfile.open(archive) as contents:
                contents.extractall(temporary, filter="data")
            extracted[package] = temporary / f"{package}-{versions[package]}"

        consumer = temporary / "consumer"
        environment = {**os.environ, "CARGO_TARGET_DIR": str(consumer / "target")}
        (consumer / "src").mkdir(parents=True)
        (consumer / "src" / "main.rs").write_text(
            (ROOT / "crates/behavior-macros/tests/fixtures/unrenamed-direct/src/lib.rs")
            .read_text()
            + "\nfn main() {}\n"
        )
        manifest = [
            '[package]',
            'name = "published-consumer"',
            'version = "0.0.0"',
            'edition = "2024"',
            '',
            '[dependencies]',
        ]
        manifest.extend(f'{package} = "{declared[package]}"' for package in PACKAGES[1:])
        manifest.extend(['', '[patch.crates-io]'])
        manifest.extend(
            f'{package} = {{ path = "../{extracted[package].name}" }}'
            for package in PACKAGES
        )
        (consumer / "Cargo.toml").write_text("\n".join(manifest) + "\n")
        subprocess.run(
            ["cargo", "check", "--manifest-path", str(consumer / "Cargo.toml")],
            cwd=temporary,
            env=environment,
            check=True,
        )
        check_published_worker(temporary, extracted["bombay-behavior-macros"])
        check_published_product(temporary, extracted["bombay-behavior-macros"])


if __name__ == "__main__":
    main()
