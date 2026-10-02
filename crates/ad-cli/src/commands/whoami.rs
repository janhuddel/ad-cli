use std::path::PathBuf;

use ldap_cli_core::{account, Result};

use crate::cli::WhoamiArgs;
use crate::APP;

pub async fn run(args: WhoamiArgs, config_override: Option<&PathBuf>) -> Result<()> {
    account::whoami(&APP, args.verify, config_override).await
}
