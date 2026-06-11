#[tokio::main]
async fn main() {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    if let Ok(parsed) = pi_coding_agent::parse_args(raw.clone()) {
        if parsed.mode == pi_coding_agent::CliMode::Rpc {
            let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            let options = pi_coding_agent::CliRunOptions {
                model_override: None,
                tools: pi_coding_agent::builtin_tools(cwd.clone()),
                register_builtins: true,
                session: pi_coding_agent::SessionRunOptions::enabled(cwd),
            };
            match pi_coding_agent::protocol::rpc::run_rpc_mode_stdio(options).await {
                Ok(()) => std::process::exit(0),
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(1);
                }
            }
        }
    }

    let output = pi_coding_agent::run_cli(raw).await;

    if !output.stdout.is_empty() {
        print!("{}", output.stdout);
    }
    if !output.stderr.is_empty() {
        eprint!("{}", output.stderr);
    }

    std::process::exit(output.exit_code);
}
