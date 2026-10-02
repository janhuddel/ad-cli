mod cli;
mod commands;
mod ldap;
mod output;

use clap::Parser;
use ldap_cli_core::App;

use cli::{Cli, Commands};

const APP: App = App {
    bin_name: "ad",
    dir_name: "ad-cli",
    stage: None,
};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let config_override = cli.config.as_ref();

    let result = match cli.command {
        Commands::Login(args) => commands::login::run(args, config_override).await,
        Commands::Logout(args) => commands::logout::run(args, config_override),
        Commands::Whoami(args) => commands::whoami::run(args, config_override).await,
        Commands::User(args) => commands::user::run(args, config_override).await,
        Commands::Groups(args) => commands::groups::run(args, config_override).await,
        Commands::Members(args) => commands::members::run(args, config_override).await,
    };

    if let Err(err) = result {
        eprintln!("Error: {err}");
        std::process::exit(1);
    }
}
