pub fn version_json() -> String {
    format!(
        r#"{{"package":"agent-harness","binary":"agent-harness","version":"{}"}}"#,
        env!("CARGO_PKG_VERSION")
    )
}
