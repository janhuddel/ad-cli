use std::path::PathBuf;

use crate::cli::LogoutArgs;
use crate::config::paths;
use crate::credentials;
use crate::error::Result;

pub fn run(args: LogoutArgs, config_override: Option<&PathBuf>) -> Result<()> {
    credentials::delete()?;
    if args.purge {
        let path = paths::config_file_path(config_override)?;
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        println!("Removed stored credentials and connection settings.");
    } else {
        println!(
            "Removed stored credentials. Connection settings kept (run 'ad login' to re-authenticate)."
        );
    }
    Ok(())
}
