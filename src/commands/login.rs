use std::path::PathBuf;

use dialoguer::{Input, Password};

use crate::cli::LoginArgs;
use crate::config::{paths, Config};
use crate::credentials;
use crate::error::{AppError, Result};
use crate::ldap;

pub async fn run(args: LoginArgs, config_override: Option<&PathBuf>) -> Result<()> {
    let host = match args.host {
        Some(h) => h,
        None => Input::new()
            .with_prompt("AD server host")
            .interact_text()
            .map_err(|e| AppError::Other(e.to_string()))?,
    };
    let base_dn = match args.base_dn {
        Some(b) => b,
        None => Input::new()
            .with_prompt("Base DN (e.g. DC=corp,DC=example,DC=com)")
            .interact_text()
            .map_err(|e| AppError::Other(e.to_string()))?,
    };
    let bind_identity = match args.bind_identity {
        Some(b) => b,
        None => Input::new()
            .with_prompt("Bind identity (UPN, DOMAIN\\user, or full DN)")
            .interact_text()
            .map_err(|e| AppError::Other(e.to_string()))?,
    };

    let password = if args.password_stdin {
        let mut buf = String::new();
        std::io::stdin().read_line(&mut buf).map_err(AppError::Io)?;
        buf.trim_end_matches(['\r', '\n']).to_string()
    } else {
        Password::new()
            .with_prompt("Password")
            .interact()
            .map_err(|e| AppError::Other(e.to_string()))?
    };

    let config = Config {
        host,
        port: args.port,
        base_dn,
        bind_identity,
        insecure_skip_verify: args.insecure_skip_verify,
    };

    // Only persist anything once a live bind has actually succeeded.
    let mut conn = ldap::connect(&config).await?;
    ldap::bind(&mut conn, &config.bind_identity, &password).await?;
    let _ = conn.unbind().await;

    config.save(config_override)?;
    credentials::save(&password)?;

    println!(
        "Logged in as {} ({}:{}). Settings stored in {}.",
        config.bind_identity,
        config.host,
        config.port,
        paths::config_dir()?.display()
    );
    Ok(())
}
