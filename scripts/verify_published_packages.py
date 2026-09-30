"""Compile the README consumer against the contents of packaged crate archives."""

import json
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
            check=True,
        )


if __name__ == "__main__":
    main()
