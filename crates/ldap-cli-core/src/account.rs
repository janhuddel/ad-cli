//! The `login` / `logout` / `whoami` flows, identical across tools apart
//! from the prompt wording and the binary name in messages.

use std::path::PathBuf;

use dialoguer::{Input, Password};

use crate::config::{self, Config};
use crate::credentials;
use crate::error::{Error, Result};
use crate::{ldap, App};

/// What the user passed on the command line; missing values are prompted.
pub struct LoginInput {
    pub host: Option<String>,
    pub port: u16,
    pub base_dn: Option<String>,
    pub bind_identity: Option<String>,
    /// Explicitly log in without a bind identity (only honoured when
    /// `LoginPrompts::allow_anonymous` is set).
    pub anonymous: bool,
    pub password_stdin: bool,
    pub insecure_skip_verify: bool,
}

/// Prompt texts, since they name directory-specific formats.
pub struct LoginPrompts {
    pub host: &'static str,
    pub base_dn: &'static str,
    pub bind_identity: &'static str,
    /// Whether the tool supports anonymous access: then an empty bind
    /// identity at the prompt means "anonymous" instead of being rejected.
    pub allow_anonymous: bool,
}

pub async fn login(
    app: &App,
    input: LoginInput,
    prompts: &LoginPrompts,
    config_override: Option<&PathBuf>,
) -> Result<()> {
    let host = prompt_unless_given(input.host, prompts.host)?;
    let base_dn = prompt_unless_given(input.base_dn, prompts.base_dn)?;
    let bind_identity = match input.bind_identity {
        Some(b) => Some(b),
        None if prompts.allow_anonymous && input.anonymous => None,
        None if prompts.allow_anonymous => {
            let b: String = Input::new()
                .with_prompt(prompts.bind_identity)
                .allow_empty(true)
                .interact_text()
                .map_err(|e| Error::Other(e.to_string()))?;
            Some(b).filter(|b| !b.trim().is_empty())
        }
        None => Some(prompt_unless_given(None, prompts.bind_identity)?),
    };

    let password = match bind_identity {
        None => None,
        Some(_) if input.password_stdin => {
            let mut buf = String::new();
            std::io::stdin().read_line(&mut buf).map_err(Error::Io)?;
            Some(buf.trim_end_matches(['\r', '\n']).to_string())
        }
        Some(_) => Some(
            Password::new()
                .with_prompt("Password")
                .interact()
                .map_err(|e| Error::Other(e.to_string()))?,
        ),
    };

    let config = Config {
        host,
        port: input.port,
        base_dn,
        bind_identity,
        insecure_skip_verify: input.insecure_skip_verify,
    };

    // Only persist anything once the connection has actually been proven:
    // a live bind, or for anonymous access a read of the base DN.
    let mut conn = ldap::connect(&config).await?;
    match (&config.bind_identity, &password) {
        (Some(identity), Some(password)) => ldap::bind(&mut conn, identity, password).await?,
        _ => ldap::check_base_dn(&mut conn, &config.base_dn).await?,
    }
    let _ = conn.unbind().await;

    config.save(app, config_override)?;
    match &password {
        Some(password) => credentials::save(app, password)?,
        // Don't leave a stale password behind when switching to anonymous.
        None => credentials::delete(app)?,
    }

    println!(
        "Logged in as {} ({}:{}). Settings stored in {}.",
        identity_label(&config),
        config.host,
        config.port,
        app.config_dir()?.display()
    );
    Ok(())
}

fn identity_label(config: &Config) -> &str {
    config.bind_identity.as_deref().unwrap_or("anonymous")
}

fn prompt_unless_given(value: Option<String>, prompt: &str) -> Result<String> {
    match value {
        Some(v) => Ok(v),
        None => Input::new()
            .with_prompt(prompt)
            .interact_text()
            .map_err(|e| Error::Other(e.to_string())),
    }
}

pub fn logout(app: &App, purge: bool, config_override: Option<&PathBuf>) -> Result<()> {
    credentials::delete(app)?;
    if purge {
        let path = app.config_file_path(config_override)?;
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        println!("Removed stored credentials and connection settings.");
    } else {
        println!(
            "Removed stored credentials. Connection settings kept (run '{}' to re-authenticate).",
            app.login_command()
        );
    }
    Ok(())
}

pub async fn whoami(app: &App, verify: bool, config_override: Option<&PathBuf>) -> Result<()> {
    let cfg = Config::load(app, config_override)?;
    if let Some(stage) = &app.stage {
        println!("Stage:         {stage}");
    }
    println!("Host:          {}:{}", cfg.host, cfg.port);
    println!("Bind identity: {}", identity_label(&cfg));
    println!("Base DN:       {}", cfg.base_dn);
    println!("Config dir:    {}", app.config_dir()?.display());

    if verify {
        let session = config::load_session(app, config_override)?;
        let mut conn = ldap::connect(&session.config).await?;
        let (check, ok) = match (&session.config.bind_identity, &session.password) {
            (Some(identity), Some(password)) => (
                ldap::bind(&mut conn, identity, password).await,
                "bind succeeded",
            ),
            _ => (
                ldap::check_base_dn(&mut conn, &session.config.base_dn).await,
                "base DN readable anonymously",
            ),
        };
        match check {
            Ok(()) => println!("Verify:        OK - {ok}."),
            Err(e) => println!("Verify:        FAILED - {e}"),
        }
        let _ = conn.unbind().await;
    }
    Ok(())
}
