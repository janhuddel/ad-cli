use std::path::PathBuf;

use ldap_cli_core::{account, Result};

use crate::cli::LogoutArgs;
use crate::APP;

pub fn run(args: LogoutArgs, config_override: Option<&PathBuf>) -> Result<()> {
    account::logout(&APP, args.purge, config_override)
}
