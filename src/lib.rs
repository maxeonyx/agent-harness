pub const PROCESS_HANDOFF_PATH: &str = "docs/process/HANDOFF.md";
pub const SOURCE_NOTES_PATH: &str = "docs/source-notes";

pub fn version_json() -> String {
    format!(
        r#"{{"package":"agent-harness","binary":"agent-harness","version":"{}"}}"#,
        env!("CARGO_PKG_VERSION")
    )
}
