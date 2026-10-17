//! The browser limb's process.
fn main() -> std::process::ExitCode {
    harness_limb::main::<harness_limb_browser::Browser>(harness_limb_browser::KIND)
}
