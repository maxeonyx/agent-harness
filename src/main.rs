fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("-h" | "--help") => print_help(),
        Some("-V" | "--version") if args.get(1).map(String::as_str) == Some("--json") => {
            print_version_json();
        }
        Some("-V" | "--version") => println!("agent-harness {}", env!("CARGO_PKG_VERSION")),
        Some(command) => {
            eprintln!("unknown command: {command}");
            eprintln!("run `agent-harness --help` for the current process state");
            std::process::exit(2);
        }
        None => print_help(),
    }
}

fn print_version_json() {
    println!("{}", agent_harness::version_json());
}

fn print_help() {
    println!(
        "\
agent-harness {}

Experimental process-steered workbench for the agent harness design.

No product harness behavior is implemented yet. Start with:
  {}

Current development rule:
  define black-box tests and disposable spikes before core implementation

Usage:
  agent-harness --help
  agent-harness --version
",
        env!("CARGO_PKG_VERSION"),
        agent_harness::PROCESS_HANDOFF_PATH
    );
}
