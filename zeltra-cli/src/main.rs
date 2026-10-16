use clap::Parser;
use rand::thread_rng;
use zeltra_cli::cli::Cli;
use zeltra_cli::commands;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let mut rng = thread_rng();

    commands::handle_command(cli.command, &mut rng).await;
}
