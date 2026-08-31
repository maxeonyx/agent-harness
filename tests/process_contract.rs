#[test]
fn binary_reports_product_help() {
    let output = assert_cmd::Command::cargo_bin("agent-harness")
        .expect("agent-harness binary should be built")
        .arg("--help")
        .output()
        .expect("agent-harness --help should run");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.contains("Evented agent harness"));
    assert!(stdout.contains("face, brain, and workspace-runtime roles"));
    assert!(!stdout.contains("docs/process/HANDOFF.md"));
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

#[test]
fn binary_rejects_trailing_arguments_and_documents_json_version() {
    for args in [
        &["--help", "unexpected"][..],
        &["--version", "unexpected"][..],
        &["--version", "--json", "unexpected"][..],
    ] {
        let output = assert_cmd::Command::cargo_bin("agent-harness")
            .expect("agent-harness binary should be built")
            .args(args)
            .output()
            .expect("agent-harness should reject trailing arguments");

        assert_eq!(output.status.code(), Some(2), "args: {args:?}");
        let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
        assert!(stderr.contains("unexpected argument: unexpected"));
        assert!(stderr.contains("agent-harness --help"));
    }

    let help = assert_cmd::Command::cargo_bin("agent-harness")
        .expect("agent-harness binary should be built")
        .arg("--help")
        .output()
        .expect("agent-harness --help should run");
    let stdout = String::from_utf8(help.stdout).expect("help stdout should be UTF-8");
    assert!(stdout.contains("agent-harness --version --json"));
}
