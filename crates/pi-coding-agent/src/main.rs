#[tokio::main]
async fn main() {
    let output = pi_coding_agent::api::cli::runner::run_cli_stdio(std::env::args().skip(1)).await;

    if !output.stdout.is_empty() {
        print!("{}", output.stdout);
    }
    if !output.stderr.is_empty() {
        eprint!("{}", output.stderr);
    }

    std::process::exit(output.exit_code);
}
