use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(
    name = "idm",
    version,
    about = "Query user details and assigned rights from the IdM over LDAP"
)]
pub struct Cli {
    /// IdM stage (environment) to query, as named at `idm login --stage`.
    /// Defaults to the default stage (see `idm stage`).
    #[arg(long, global = true, env = "IDM_STAGE", value_name = "NAME")]
    pub stage: Option<String>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Set up a stage: store its connection settings (and credentials, unless
    /// anonymous). The first stage set up becomes the default.
    Login(LoginArgs),
    /// Remove a stage's stored credentials (with --purge the whole stage)
    Logout(LogoutArgs),
    /// List the stages that are set up, or change the default stage
    Stage(StageArgs),
    /// Show the currently stored connection settings
    Whoami(WhoamiArgs),
    /// Show all attributes of a single IdM user
    User(UserArgs),
    /// Show the rights assigned to a single IdM user
    Rights(RightsArgs),
}

#[derive(Args)]
pub struct LoginArgs {
    #[arg(long)]
    pub host: Option<String>,
    #[arg(long, default_value_t = 636)]
    pub port: u16,
    #[arg(long = "base-dn")]
    pub base_dn: Option<String>,
    /// Bind DN; omit (or leave empty at the prompt) for anonymous access.
    #[arg(long = "bind-identity", conflicts_with = "anonymous")]
    pub bind_identity: Option<String>,
    /// Use anonymous access without prompting for a bind DN.
    #[arg(long)]
    pub anonymous: bool,
    /// Read the password from stdin instead of an interactive masked prompt
    /// (for scripted/CI logins). Never pass the password as a bare argument.
    #[arg(long = "password-stdin")]
    pub password_stdin: bool,
    /// Skip LDAPS certificate verification. Only for lab/self-signed servers —
    /// this disables protection against a man-in-the-middle.
    #[arg(long = "insecure-skip-verify")]
    pub insecure_skip_verify: bool,
}

#[derive(Args)]
pub struct LogoutArgs {
    /// Also remove the stored connection settings, i.e. the whole stage.
    #[arg(long)]
    pub purge: bool,
}

#[derive(Args)]
pub struct StageArgs {
    #[command(subcommand)]
    pub command: Option<StageCommand>,
}

#[derive(Subcommand)]
pub enum StageCommand {
    /// List the stages that are set up (the default)
    List,
    /// Make NAME the stage used when --stage is not given
    Default { name: String },
}

#[derive(Args)]
pub struct WhoamiArgs {
    /// Connect with the stored settings to confirm they still work.
    #[arg(long)]
    pub verify: bool,
}

#[derive(Args)]
pub struct UserArgs {
    /// User ID (cn/uid), mail, or name words to search for; several matches
    /// open a picker
    #[arg(required = true)]
    pub identifier: Vec<String>,
    #[arg(long, value_enum, default_value_t = UserOutputFormat::Compact)]
    pub output: UserOutputFormat,
    /// Also list every raw attribute the server returned
    #[arg(long)]
    pub all: bool,
}

#[derive(Copy, Clone, ValueEnum)]
pub enum UserOutputFormat {
    Compact,
    Json,
}

#[derive(Args)]
pub struct RightsArgs {
    /// User ID (cn/uid), mail, or name words to search for; several matches
    /// open a picker
    #[arg(required = true)]
    pub identifier: Vec<String>,
    /// Defaults to an interactive fuzzy filter on a TTY, or a plain table otherwise.
    #[arg(long, value_enum)]
    pub output: Option<RightsOutputFormat>,
    /// Force plain output even when stdout is a TTY.
    #[arg(long = "no-interactive")]
    pub no_interactive: bool,
}

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
pub enum RightsOutputFormat {
    Table,
    Csv,
    Json,
}
