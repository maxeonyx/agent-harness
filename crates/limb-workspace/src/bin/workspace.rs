//! The workspace limb's process.
fn main() -> std::process::ExitCode {
    harness_limb::main::<harness_limb_workspace::Workspace>(harness_limb_workspace::KIND)
}
