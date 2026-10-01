use std::path::PathBuf;

use crate::cli::WhoamiArgs;
use crate::config::{self, paths, Config};
use crate::error::Result;
use crate::ldap;

pub async fn run(args: WhoamiArgs, config_override: Option<&PathBuf>) -> Result<()> {
    let cfg = Config::load(config_override)?;
    println!("Host:          {}:{}", cfg.host, cfg.port);
    println!("Bind identity: {}", cfg.bind_identity);
    println!("Base DN:       {}", cfg.base_dn);
    println!("Config dir:    {}", paths::config_dir()?.display());

    if args.verify {
        let session = config::load_session(config_override)?;
        let mut conn = ldap::connect(&session.config).await?;
        match ldap::bind(&mut conn, &session.config.bind_identity, &session.password).await {
            Ok(()) => println!("Verify:        OK - bind succeeded."),
            Err(e) => println!("Verify:        FAILED - {e}"),
        }
        let _ = conn.unbind().await;
    }
    Ok(())
}
