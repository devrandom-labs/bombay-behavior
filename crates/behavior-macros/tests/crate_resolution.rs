use std::path::PathBuf;
use std::process::Command;

fn fixture_manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("Cargo.toml")
}

fn cargo_check(packages: &[&str]) -> std::process::Output {
    let mut command = Command::new(env!("CARGO"));
    command
        .arg("check")
        .arg("--offline")
        .arg("--locked")
        .arg("--manifest-path")
        .arg(fixture_manifest());
    for package in packages {
        command.arg("--package").arg(package);
    }
    command.output().expect("fixture cargo check must start")
}

#[test]
fn direct_and_facade_dependency_paths_resolve_with_renames() {
    let output = cargo_check(&["facade-only", "renamed-facade", "direct-and-facade"]);
    assert!(
        output.status.success(),
        "fixture compilation failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn missing_behavior_and_facade_dependencies_report_the_contract_error() {
    let output = cargo_check(&["missing-dependency"]);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("could not resolve `bombay-behavior` directly or through `bombay-rs`"),
        "unexpected compiler error:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
