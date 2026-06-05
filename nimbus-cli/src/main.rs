mod cli;
mod commands;

use clap::Parser;
use cli::Cli;
use rand::thread_rng;

fn main() {
    let cli = Cli::parse();
    let mut rng = thread_rng();
    
    commands::handle_command(cli.command, &mut rng);
}
