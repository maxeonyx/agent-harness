fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("-h" | "--help") => match args.get(1) {
            Some(unexpected) => reject_unexpected_argument(unexpected),
            None => print_help(),
        },
        Some("-V" | "--version") if args.get(1).map(String::as_str) == Some("--json") => {
            match args.get(2) {
                Some(unexpected) => reject_unexpected_argument(unexpected),
                None => print_version_json(),
            }
        }
        Some("-V" | "--version") => match args.get(1) {
            Some(unexpected) => reject_unexpected_argument(unexpected),
            None => println!("agent-harness {}", env!("CARGO_PKG_VERSION")),
        },
        Some(command) => {
            eprintln!("unknown command: {command}");
            eprintln!("run `agent-harness --help` for usage");
            std::process::exit(2);
        }
        None => print_help(),
    }
}

fn reject_unexpected_argument(argument: &str) -> ! {
    eprintln!("unexpected argument: {argument}");
    eprintln!("run `agent-harness --help` for usage");
    std::process::exit(2);
}

fn print_version_json() {
    println!("{}", agent_harness::version_json());
}

fn print_help() {
    println!(
        "\
agent-harness {}

Evented agent harness for coordinated face, brain, and workspace-runtime roles.

Usage:
  agent-harness --help
  agent-harness --version
  agent-harness --version --json
",
        env!("CARGO_PKG_VERSION")
    );
}
