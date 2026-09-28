//! Verdict tool for the on-demand mutation-testing derivation.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

use serde::Deserialize;

#[derive(Deserialize)]
struct Report {
    outcomes: Vec<Outcome>,
}

#[derive(Deserialize)]
struct Outcome {
    summary: Summary,
    scenario: Scenario,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
enum Summary {
    Success,
    CaughtMutant,
    MissedMutant,
    Unviable,
    Timeout,
    Failure,
}

#[derive(Deserialize)]
enum Scenario {
    Baseline,
    Mutant(Mutant),
}

#[derive(Deserialize)]
struct Mutant {
    name: String,
    file: String,
    function: Option<Function>,
}

#[derive(Deserialize)]
struct Function {
    function_name: String,
}

#[derive(Deserialize)]
struct Baseline {
    floors: BTreeMap<String, usize>,
    known_zero_viable: Vec<String>,
}

#[derive(Default)]
struct Tally {
    total: usize,
    viable: usize,
    missed: usize,
    timeout: usize,
}

fn key(file: &str, function: Option<&Function>) -> String {
    function.map_or_else(
        || format!("{file}::<module>"),
        |value| format!("{file}::{}", value.function_name),
    )
}

fn read<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let text = fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    serde_json::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))
}

fn tallies(report: &Report) -> Result<BTreeMap<String, Tally>, String> {
    let mut result = BTreeMap::<String, Tally>::new();
    for outcome in &report.outcomes {
        let Scenario::Mutant(mutant) = &outcome.scenario else {
            continue;
        };
        let tally = result
            .entry(key(&mutant.file, mutant.function.as_ref()))
            .or_default();
        tally.total += 1;
        match outcome.summary {
            Summary::CaughtMutant => tally.viable += 1,
            Summary::MissedMutant => {
                tally.viable += 1;
                tally.missed += 1;
            }
            Summary::Timeout => {
                tally.viable += 1;
                tally.timeout += 1;
            }
            Summary::Unviable => {}
            Summary::Success | Summary::Failure => {
                return Err(format!("{}: failed or invalid mutant outcome", mutant.name));
            }
        }
    }
    Ok(result)
}

fn campaign_tallies(
    report: &Report,
    candidates: &[Mutant],
) -> Result<BTreeMap<String, Tally>, String> {
    let baselines: Vec<_> = report
        .outcomes
        .iter()
        .filter(|outcome| matches!(outcome.scenario, Scenario::Baseline))
        .collect();
    if baselines.len() != 1 || baselines[0].summary != Summary::Success {
        return Err("exactly one successful unmutated baseline is required".into());
    }
    if candidates.is_empty() {
        return Err("no mutants were selected".into());
    }
    let mut expected = BTreeMap::new();
    for candidate in candidates {
        if expected
            .insert(
                &candidate.name,
                key(&candidate.file, candidate.function.as_ref()),
            )
            .is_some()
        {
            return Err(format!("duplicate candidate: {}", candidate.name));
        }
    }
    let mut observed = BTreeSet::new();
    for outcome in &report.outcomes {
        let Scenario::Mutant(mutant) = &outcome.scenario else {
            continue;
        };
        let Some(expected_key) = expected.get(&mutant.name) else {
            return Err(format!("unknown mutant outcome: {}", mutant.name));
        };
        if expected_key != &key(&mutant.file, mutant.function.as_ref()) {
            return Err(format!("mutant identity mismatch: {}", mutant.name));
        }
        if !observed.insert(&mutant.name) {
            return Err(format!("duplicate mutant outcome: {}", mutant.name));
        }
    }
    if let Some(missing) = expected.keys().find(|name| !observed.contains(*name)) {
        return Err(format!("missing mutant outcome: {missing}"));
    }
    tallies(report)
}

fn emit_baseline(report: &Report, candidates: &[Mutant]) -> Result<String, String> {
    let mut floors = BTreeMap::new();
    let mut known_zero_viable = Vec::new();
    for (key, tally) in campaign_tallies(report, candidates)? {
        if tally.viable == 0 {
            known_zero_viable.push(key);
        } else {
            floors.insert(key, tally.viable);
        }
    }
    serde_json::to_string_pretty(&serde_json::json!({
        "floors": floors,
        "known_zero_viable": known_zero_viable,
    }))
    .map_err(|error| error.to_string())
}

fn check(output: &Path, baseline_path: &Path) -> Result<(), String> {
    let report: Report = read(&output.join("outcomes.json"))?;
    let candidates: Vec<Mutant> = read(&output.join("mutants.json"))?;
    let tallies = campaign_tallies(&report, &candidates)?;
    let baseline: Baseline = read(baseline_path)?;
    let expected: BTreeSet<_> = candidates
        .iter()
        .map(|candidate| key(&candidate.file, candidate.function.as_ref()))
        .collect();
    let known_zero: BTreeSet<_> = baseline.known_zero_viable.iter().collect();
    let mut failures = Vec::new();
    if known_zero.len() != baseline.known_zero_viable.len() {
        failures.push("duplicate known-zero-viable entry".to_owned());
    }
    for key in &known_zero {
        if baseline.floors.contains_key(*key) {
            failures.push(format!("conflicting baseline entries: {key}"));
        } else if !expected.contains(*key) {
            failures.push(format!("stale known-zero-viable entry: {key}"));
        }
    }
    for (key, floor) in &baseline.floors {
        if *floor == 0 {
            failures.push(format!("invalid zero floor: {key}"));
        } else if !expected.contains(key) {
            failures.push(format!("stale floor: {key}"));
        }
    }
    for (key, tally) in &tallies {
        if tally.missed > 0 {
            failures.push(format!("{key}: {} survivor(s)", tally.missed));
        }
        if tally.timeout > 0 {
            failures.push(format!("{key}: {} timeout(s)", tally.timeout));
        }
        if let Some(floor) = baseline.floors.get(key) {
            if tally.viable < *floor {
                failures.push(format!(
                    "{key}: viability collapsed to {} below {floor}",
                    tally.viable
                ));
            }
        } else if !known_zero.contains(key) {
            failures.push(format!("unaccounted: {key}"));
        }
    }
    let viable: usize = tallies.values().map(|tally| tally.viable).sum();
    let total: usize = tallies.values().map(|tally| tally.total).sum();
    println!("mutation coverage: {viable} viable / {total} total");
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("\n"))
    }
}

fn run() -> Result<(), String> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    match args.as_slice() {
        [mode, output] if mode == "emit-baseline" => {
            let output = Path::new(output);
            let report = read(&output.join("outcomes.json"))?;
            let candidates: Vec<Mutant> = read(&output.join("mutants.json"))?;
            println!("{}", emit_baseline(&report, &candidates)?);
            Ok(())
        }
        [mode, output, baseline] if mode == "check" => {
            check(Path::new(output), Path::new(baseline))
        }
        _ => Err("usage: behavior-mutants-gate <emit-baseline OUT | check OUT BASELINE>".into()),
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("behavior-mutants-gate FAIL: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{check, emit_baseline};
    use std::fs;

    fn report(directory: &std::path::Path, outcomes: serde_json::Value) -> std::path::PathBuf {
        fs::write(directory.join("outcomes.json"), outcomes.to_string()).unwrap();
        fs::write(
            directory.join("mutants.json"),
            serde_json::json!([
                {"name":"first", "file":"a.rs", "function":{"function_name":"f"}},
                {"name":"second", "file":"a.rs", "function":{"function_name":"f"}}
            ])
            .to_string(),
        )
        .unwrap();
        let baseline = directory.join("baseline.json");
        fs::write(
            &baseline,
            r#"{"floors":{"a.rs::f":1},"known_zero_viable":[]}"#,
        )
        .unwrap();
        baseline
    }

    #[test]
    fn missing_unmutated_baseline_cannot_certify_mutants() {
        let directory = scratch("missing-baseline");
        let baseline = report(
            &directory,
            serde_json::json!({"outcomes":[
                {"summary":"CaughtMutant","scenario":{"Mutant":{"name":"first","file":"a.rs","function":{"function_name":"f"}}}},
                {"summary":"Unviable","scenario":{"Mutant":{"name":"second","file":"a.rs","function":{"function_name":"f"}}}}
            ]}),
        );
        assert!(
            check(&directory, &baseline)
                .unwrap_err()
                .contains("baseline")
        );
    }

    #[test]
    fn failed_mutant_cannot_count_as_completed_campaign() {
        let directory = scratch("failed-mutant");
        let baseline = report(
            &directory,
            serde_json::json!({"outcomes":[
                {"summary":"Success","scenario":"Baseline"},
                {"summary":"CaughtMutant","scenario":{"Mutant":{"name":"first","file":"a.rs","function":{"function_name":"f"}}}},
                {"summary":"Failure","scenario":{"Mutant":{"name":"second","file":"a.rs","function":{"function_name":"f"}}}}
            ]}),
        );
        assert!(check(&directory, &baseline).unwrap_err().contains("failed"));
    }

    #[test]
    fn viable_floor_cannot_collapse_to_only_unviable_mutants() {
        let directory = scratch("viability-collapse");
        let baseline = report(
            &directory,
            serde_json::json!({"outcomes":[
                {"summary":"Success","scenario":"Baseline"},
                {"summary":"Unviable","scenario":{"Mutant":{"name":"first","file":"a.rs","function":{"function_name":"f"}}}},
                {"summary":"Unviable","scenario":{"Mutant":{"name":"second","file":"a.rs","function":{"function_name":"f"}}}}
            ]}),
        );
        assert!(
            check(&directory, &baseline)
                .unwrap_err()
                .contains("viability collapsed")
        );
    }

    #[test]
    fn duplicate_outcome_cannot_replace_a_distinct_candidate() {
        let directory = scratch("duplicate-outcome");
        let baseline = report(
            &directory,
            serde_json::json!({"outcomes":[
                {"summary":"Success","scenario":"Baseline"},
                {"summary":"CaughtMutant","scenario":{"Mutant":{"name":"first","file":"a.rs","function":{"function_name":"f"}}}},
                {"summary":"Unviable","scenario":{"Mutant":{"name":"first","file":"a.rs","function":{"function_name":"f"}}}}
            ]}),
        );
        assert!(
            check(&directory, &baseline)
                .unwrap_err()
                .contains("duplicate")
        );
    }

    #[test]
    fn selected_mutants_require_exactly_their_own_terminal_outcome() {
        let first = serde_json::json!({"Mutant":{"name":"first","file":"a.rs","function":{"function_name":"f"}}});
        let second = serde_json::json!({"Mutant":{"name":"second","file":"a.rs","function":{"function_name":"f"}}});
        let foreign = serde_json::json!({"Mutant":{"name":"foreign","file":"a.rs","function":{"function_name":"f"}}});
        for (case, outcomes, expected) in [
            (
                "missing-outcome",
                vec![serde_json::json!({"summary":"CaughtMutant","scenario":first.clone()})],
                "missing mutant outcome",
            ),
            (
                "unknown-outcome",
                vec![
                    serde_json::json!({"summary":"CaughtMutant","scenario":first.clone()}),
                    serde_json::json!({"summary":"Unviable","scenario":foreign}),
                ],
                "unknown mutant outcome",
            ),
            (
                "timeout",
                vec![
                    serde_json::json!({"summary":"CaughtMutant","scenario":first}),
                    serde_json::json!({"summary":"Timeout","scenario":second}),
                ],
                "timeout",
            ),
        ] {
            let directory = scratch(case);
            let mut outcomes = outcomes;
            outcomes.insert(
                0,
                serde_json::json!({"summary":"Success","scenario":"Baseline"}),
            );
            let baseline = report(&directory, serde_json::json!({"outcomes":outcomes}));
            assert!(check(&directory, &baseline).unwrap_err().contains(expected));
        }
    }

    #[test]
    fn stale_baseline_entries_and_malformed_candidate_are_rejected() {
        let directory = scratch("stale-baseline");
        let baseline = report(
            &directory,
            serde_json::json!({"outcomes":[
                {"summary":"Success","scenario":"Baseline"},
                {"summary":"CaughtMutant","scenario":{"Mutant":{"name":"first","file":"a.rs","function":{"function_name":"f"}}}},
                {"summary":"Unviable","scenario":{"Mutant":{"name":"second","file":"a.rs","function":{"function_name":"f"}}}}
            ]}),
        );
        fs::write(
            &baseline,
            r#"{"floors":{"a.rs::f":1},"known_zero_viable":["stale.rs::g"]}"#,
        )
        .unwrap();
        assert!(check(&directory, &baseline).unwrap_err().contains("stale"));
        fs::write(directory.join("mutants.json"), r#"[{"file":"a.rs"}]"#).unwrap();
        assert!(
            check(&directory, &baseline)
                .unwrap_err()
                .contains("missing field `name`")
        );
    }

    #[test]
    fn duplicate_baseline_and_candidate_names_are_rejected() {
        let directory = scratch("duplicate-baseline");
        let baseline = report(
            &directory,
            serde_json::json!({"outcomes":[
                {"summary":"Success","scenario":"Baseline"},
                {"summary":"Success","scenario":"Baseline"},
                {"summary":"CaughtMutant","scenario":{"Mutant":{"name":"first","file":"a.rs","function":{"function_name":"f"}}}},
                {"summary":"Unviable","scenario":{"Mutant":{"name":"second","file":"a.rs","function":{"function_name":"f"}}}}
            ]}),
        );
        assert!(
            check(&directory, &baseline)
                .unwrap_err()
                .contains("baseline")
        );
        fs::write(
            directory.join("outcomes.json"),
            serde_json::json!({"outcomes":[
                {"summary":"Success","scenario":"Baseline"},
                {"summary":"CaughtMutant","scenario":{"Mutant":{"name":"first","file":"a.rs","function":{"function_name":"f"}}}},
                {"summary":"Unviable","scenario":{"Mutant":{"name":"second","file":"a.rs","function":{"function_name":"f"}}}}
            ]})
            .to_string(),
        )
        .unwrap();
        fs::write(
            directory.join("mutants.json"),
            r#"[{"name":"first","file":"a.rs"},{"name":"first","file":"a.rs"}]"#,
        )
        .unwrap();
        assert!(
            check(&directory, &baseline)
                .unwrap_err()
                .contains("duplicate candidate")
        );
    }

    fn scratch(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "behavior-mutants-gate-{name}-{}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create gate scratch directory");
        path
    }

    #[test]
    fn clean_complete_run_passes_the_ratchet() {
        let directory = scratch("clean");
        fs::write(directory.join("outcomes.json"), r#"{"outcomes":[{"summary":"Success","scenario":"Baseline"},{"summary":"CaughtMutant","scenario":{"Mutant":{"name":"first","file":"a.rs","function":{"function_name":"f"}}}},{"summary":"Unviable","scenario":{"Mutant":{"name":"second","file":"a.rs","function":{"function_name":"f"}}}}]}"#).unwrap();
        fs::write(directory.join("mutants.json"), r#"[{"name":"first","file":"a.rs","function":{"function_name":"f"}},{"name":"second","file":"a.rs","function":{"function_name":"f"}}]"#).unwrap();
        let baseline = directory.join("baseline.json");
        fs::write(
            &baseline,
            r#"{"floors":{"a.rs::f":1},"known_zero_viable":[]}"#,
        )
        .unwrap();
        check(&directory, &baseline).expect("complete clean run passes");
    }

    #[test]
    fn survivor_fails_even_when_viability_meets_the_floor() {
        let directory = scratch("survivor");
        fs::write(directory.join("outcomes.json"), r#"{"outcomes":[{"summary":"Success","scenario":"Baseline"},{"summary":"MissedMutant","scenario":{"Mutant":{"name":"first","file":"a.rs","function":{"function_name":"f"}}}}]}"#).unwrap();
        fs::write(
            directory.join("mutants.json"),
            r#"[{"name":"first","file":"a.rs","function":{"function_name":"f"}}]"#,
        )
        .unwrap();
        let baseline = directory.join("baseline.json");
        fs::write(
            &baseline,
            r#"{"floors":{"a.rs::f":1},"known_zero_viable":[]}"#,
        )
        .unwrap();
        assert!(
            check(&directory, &baseline)
                .unwrap_err()
                .contains("survivor")
        );
    }

    #[test]
    fn seeding_rejects_a_failed_unmutated_baseline() {
        let report =
            serde_json::from_str(r#"{"outcomes":[{"summary":"Failure","scenario":"Baseline"}]}"#)
                .unwrap();
        let candidates: Vec<super::Mutant> = serde_json::from_str(
            r#"[{"name":"first","file":"a.rs","function":{"function_name":"f"}}]"#,
        )
        .unwrap();
        assert!(
            emit_baseline(&report, &candidates)
                .unwrap_err()
                .contains("baseline")
        );
    }
}
