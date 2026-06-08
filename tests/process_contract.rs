use std::fs;
use std::path::Path;

#[test]
fn imported_source_notes_are_preserved() {
    let expected = [
        "agent-harness-design.md",
        "agent-hierarchy.md",
        "analytics.md",
        "compaction.md",
        "configuration-model.md",
        "context-and-agent-loop.md",
        "open-questions.md",
        "process.md",
        "requirements.md",
        "tech.md",
        "user-turn.md",
    ];

    for file in expected {
        assert!(
            Path::new("docs/source-notes").join(file).is_file(),
            "missing imported source note: {file}"
        );
    }
}

#[test]
fn process_handoff_records_current_loop_and_boundaries() {
    let handoff = fs::read_to_string("docs/process/HANDOFF.md")
        .expect("docs/process/HANDOFF.md should be readable");

    assert!(handoff.contains("Active loop:"));
    assert!(handoff.contains("Pre-spike A"));
    assert!(handoff.contains("experiments/"));
    assert!(handoff.contains("Do not build a real provider adapter"));
}

#[test]
fn experiments_are_marked_disposable() {
    let readme =
        fs::read_to_string("experiments/README.md").expect("experiments/README.md should exist");

    assert!(readme.contains("Disposable spikes"));
    assert!(readme.contains("Core code must not depend on experiment internals"));
}

#[test]
fn process_templates_exist_for_resumable_stewardship() {
    for file in [
        "docs/process/HANDOFF_TEMPLATE.md",
        "docs/process/SPIKE_OUTCOME_TEMPLATE.md",
        "docs/process/SPIKE_RULES.md",
    ] {
        assert!(
            Path::new(file).is_file(),
            "missing process template: {file}"
        );
    }
}

#[test]
fn core_does_not_import_disposable_experiments() {
    let src_dir = Path::new("src");
    for entry in fs::read_dir(src_dir).expect("src directory should be readable") {
        let entry = entry.expect("src entry should be readable");
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }

        let content = fs::read_to_string(&path).expect("Rust source should be readable");
        assert!(
            !content.contains("experiments"),
            "{} imports or references disposable experiment code",
            path.display()
        );
    }
}

#[test]
fn binary_reports_process_state_instead_of_placeholder_output() {
    let output = assert_cmd::Command::cargo_bin("agent-harness")
        .expect("agent-harness binary should be built")
        .arg("--help")
        .output()
        .expect("agent-harness --help should run");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.contains("process-steered"));
    assert!(stdout.contains("docs/process/HANDOFF.md"));
}

#[test]
fn binary_reports_plain_and_json_versions() {
    let plain = assert_cmd::Command::cargo_bin("agent-harness")
        .expect("agent-harness binary should be built")
        .arg("--version")
        .output()
        .expect("agent-harness --version should run");
    assert!(plain.status.success());
    assert_eq!(
        String::from_utf8(plain.stdout).expect("plain version stdout should be UTF-8"),
        format!("agent-harness {}\n", env!("CARGO_PKG_VERSION"))
    );

    let json = assert_cmd::Command::cargo_bin("agent-harness")
        .expect("agent-harness binary should be built")
        .args(["--version", "--json"])
        .output()
        .expect("agent-harness --version --json should run");
    assert!(json.status.success());
    let stdout = String::from_utf8(json.stdout).expect("json version stdout should be UTF-8");
    assert!(stdout.contains(r#""package":"agent-harness""#));
    assert!(stdout.contains(r#""binary":"agent-harness""#));
    assert!(stdout.contains(&format!(r#""version":"{}""#, env!("CARGO_PKG_VERSION"))));
}
