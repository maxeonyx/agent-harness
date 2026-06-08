#[test]
fn tdd_ratchet_gatekeeper() {
    if std::env::var("TDD_RATCHET").is_err() {
        panic!(
            "Run `cargo ratchet` instead of `cargo test` for the core agent-harness package.\n\
             Disposable spikes under experiments/ may define their own local test workflow."
        );
    }
}
