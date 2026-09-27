use clap::Parser;
use std::process::ExitCode;
use translate_rs::cli::{run_cli, Cli};

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    if let Err(e) = run_cli(cli).await {
        eprintln!("translate: {e}");
        return ExitCode::from(e.exit_code() as u8);
    }
    ExitCode::SUCCESS
}
