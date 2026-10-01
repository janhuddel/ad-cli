use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(
    name = "ad",
    version,
    about = "Query Active Directory user and group information over LDAP"
)]
pub struct Cli {
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Override the default config file location (~/.config/ad-cli/config.toml
    /// on Linux, %APPDATA%\ad-cli\config.toml on Windows).
    #[arg(long, global = true, value_name = "PATH")]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Log in to Active Directory and store connection settings + credentials
    Login(LoginArgs),
    /// Remove stored credentials (and optionally connection settings)
    Logout(LogoutArgs),
    /// Show the currently stored connection identity
    Whoami(WhoamiArgs),
    /// Show details for a single AD user
    User(UserArgs),
    /// Show group memberships for a single AD user
    Groups(GroupsArgs),
}

#[derive(Args)]
pub struct LoginArgs {
    #[arg(long)]
    pub host: Option<String>,
    #[arg(long, default_value_t = 636)]
    pub port: u16,
    #[arg(long = "base-dn")]
    pub base_dn: Option<String>,
    #[arg(long = "bind-identity")]
    pub bind_identity: Option<String>,
    /// Read the password from stdin instead of an interactive masked prompt
    /// (for scripted/CI logins). Never pass the password as a bare argument.
    #[arg(long = "password-stdin")]
    pub password_stdin: bool,
    /// Skip LDAPS certificate verification. Only for lab/self-signed ADs —
    /// this disables protection against a man-in-the-middle.
    #[arg(long = "insecure-skip-verify")]
    pub insecure_skip_verify: bool,
}

#[derive(Args)]
pub struct LogoutArgs {
    /// Also remove the stored connection settings, not just credentials.
    #[arg(long)]
    pub purge: bool,
}

#[derive(Args)]
pub struct WhoamiArgs {
    /// Perform a live bind with the stored credentials to confirm they still work.
    #[arg(long)]
    pub verify: bool,
}

#[derive(Args)]
pub struct UserArgs {
    /// sAMAccountName or userPrincipalName of the target user
    pub identifier: String,
    /// `compact` is a borderless summary; `table` is the full bordered table.
    #[arg(long, value_enum, default_value_t = UserOutputFormat::Compact)]
    pub output: UserOutputFormat,
}

#[derive(Copy, Clone, ValueEnum)]
pub enum UserOutputFormat {
    Compact,
    Table,
    Json,
}

#[derive(Args)]
pub struct GroupsArgs {
    /// sAMAccountName or userPrincipalName of the target user
    pub identifier: String,
    /// Resolve nested/transitive membership too, not just direct memberOf.
    #[arg(long)]
    pub recursive: bool,
    /// Defaults to an interactive fuzzy filter on a TTY, or a plain table otherwise.
    #[arg(long, value_enum)]
    pub output: Option<GroupsOutputFormat>,
    /// Force plain output even when stdout is a TTY.
    #[arg(long = "no-interactive")]
    pub no_interactive: bool,
}

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
pub enum GroupsOutputFormat {
    Table,
    Csv,
    Json,
}
