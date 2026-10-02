mod cli;
mod commands;
mod directory;
mod output;
mod stages;

use clap::Parser;

use cli::{Cli, Commands};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let stage = cli.stage;

    let result = match cli.command {
        Commands::Login(args) => commands::login(args, stage).await,
        Commands::Logout(args) => commands::logout(args, stage),
        Commands::Stage(args) => commands::stage(args),
        Commands::Whoami(args) => commands::whoami(args, stage).await,
        Commands::User(args) => commands::user(args, stage).await,
        Commands::Rights(args) => commands::rights(args, stage).await,
    };

    if let Err(err) = result {
        eprintln!("Error: {err}");
        std::process::exit(1);
    }
}
