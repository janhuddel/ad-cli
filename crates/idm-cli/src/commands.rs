use std::collections::HashMap;

use ldap3::Ldap;
use ldap_cli_core::account::{self, LoginInput, LoginPrompts};
use ldap_cli_core::config::{self, Config, Session};
use ldap_cli_core::{ldap, Error, Result};

use crate::cli::{
    LoginArgs, LogoutArgs, RightsArgs, StageArgs, StageCommand, UserArgs, UserOutputFormat,
    WhoamiArgs,
};
use crate::directory::{self, UserEntry};
use crate::output::{rights_view, user_picker, user_view};
use crate::stages;

const PROMPTS: LoginPrompts = LoginPrompts {
    host: "IdM LDAP server host",
    base_dn: "Base DN of the users (e.g. ou=users,o=example)",
    bind_identity: "Bind DN (leave empty for anonymous access)",
    allow_anonymous: true,
};

pub async fn login(args: LoginArgs, stage: Option<String>) -> Result<()> {
    let app = stages::select_for_login(stage)?;
    let input = LoginInput {
        host: args.host,
        port: args.port,
        base_dn: args.base_dn,
        bind_identity: args.bind_identity,
        anonymous: args.anonymous,
        password_stdin: args.password_stdin,
        insecure_skip_verify: args.insecure_skip_verify,
    };
    account::login(&app, input, &PROMPTS, None).await?;

    // The first stage becomes the default (also if the old default is gone).
    let name = app.stage.as_deref().unwrap_or_default();
    let configured = stages::list()?;
    if !stages::default_stage()?.is_some_and(|d| configured.contains(&d)) {
        stages::set_default(name)?;
        println!("'{name}' is now the default stage.");
    }
    Ok(())
}

pub fn logout(args: LogoutArgs, stage: Option<String>) -> Result<()> {
    let app = stages::select(stage)?;
    account::logout(&app, args.purge, None)?;
    if args.purge {
        let name = app.stage.as_deref().unwrap_or_default();
        stages::clear_default_if(name)?;
        // Only remove the stage directory if nothing else is left in it.
        let _ = std::fs::remove_dir(app.config_dir()?);
    }
    Ok(())
}

pub fn stage(args: StageArgs) -> Result<()> {
    match args.command.unwrap_or(StageCommand::List) {
        StageCommand::List => list_stages(),
        StageCommand::Default { name } => {
            stages::validate_name(&name)?;
            if !stages::list()?.contains(&name) {
                return Err(Error::Other(format!(
                    "stage '{name}' is not set up. Run 'idm login --stage {name}' first."
                )));
            }
            stages::set_default(&name)?;
            println!("'{name}' is now the default stage.");
            Ok(())
        }
    }
}

fn list_stages() -> Result<()> {
    let names = stages::list()?;
    if names.is_empty() {
        println!("No stages set up yet. Run 'idm login --stage <name>' to set one up.");
        return Ok(());
    }
    let default = stages::default_stage()?;
    let width = names.iter().map(|n| n.chars().count()).max().unwrap_or(0);
    for name in names {
        let app = stages::select(Some(name.clone()))?;
        let marker = if default.as_deref() == Some(&name) {
            '*'
        } else {
            ' '
        };
        let detail = match Config::load(&app, None) {
            Ok(c) => format!(
                "{}:{}  {}  {}",
                c.host,
                c.port,
                c.base_dn,
                c.bind_identity.as_deref().unwrap_or("anonymous")
            ),
            Err(e) => format!("(unreadable: {e})"),
        };
        println!("{marker} {name:width$}  {detail}");
    }
    Ok(())
}

pub async fn whoami(args: WhoamiArgs, stage: Option<String>) -> Result<()> {
    let app = stages::select(stage)?;
    account::whoami(&app, args.verify, None).await
}

pub async fn user(args: UserArgs, stage: Option<String>) -> Result<()> {
    let app = stages::select(stage)?;
    let session = config::load_session(&app, None)?;
    let related = matches!(args.output, UserOutputFormat::Compact);
    let Some((user, names)) = load_user(&session, &args.identifier.join(" "), related).await?
    else {
        return Ok(());
    };
    let stage = app.stage.as_deref().unwrap_or_default();
    match args.output {
        UserOutputFormat::Compact => user_view::render_compact(&user, stage, &names, args.all),
        UserOutputFormat::Json => user_view::render_json(&user)?,
    }
    Ok(())
}

pub async fn rights(args: RightsArgs, stage: Option<String>) -> Result<()> {
    let app = stages::select(stage)?;
    let session = config::load_session(&app, None)?;
    let Some((user, _)) = load_user(&session, &args.identifier.join(" "), false).await? else {
        return Ok(());
    };
    let stage = app.stage.as_deref().unwrap_or_default();
    rights_view::display(&user, stage, args.output, args.no_interactive)
}

/// Resolves the input to one user and reads the full entry; with `related`
/// also the names of the people it references (manager, last modifier),
/// keyed by lowercased ID. `None` means the pick was aborted.
async fn load_user(
    session: &Session,
    input: &str,
    related: bool,
) -> Result<Option<(UserEntry, HashMap<String, String>)>> {
    let base_dn = &session.config.base_dn;
    let mut conn = ldap::open(session).await?;
    let result = match resolve_user(&mut conn, base_dn, input).await {
        Ok(Some(dn)) => match directory::read_user(&mut conn, &dn).await {
            Ok(user) => {
                let names = if related {
                    let ids: Vec<&str> = [directory::MANAGER_ATTR, directory::MODIFIER_ATTR]
                        .iter()
                        .filter_map(|a| user.first(a))
                        .collect();
                    directory::lookup_names(&mut conn, base_dn, &ids).await
                } else {
                    HashMap::new()
                };
                Ok(Some((user, names)))
            }
            Err(e) => Err(e),
        },
        other => other.map(|_| None),
    };
    let _ = conn.unbind().await;
    result
}

/// Turns what the user typed into a user DN: an exact match on one of the
/// ID attributes is used as-is, otherwise a name search whose hits go to
/// the picker (a single hit is used directly).
async fn resolve_user(ldap: &mut Ldap, base_dn: &str, input: &str) -> Result<Option<String>> {
    let term = input.trim();
    let (exact, _) = directory::search_users(ldap, base_dn, &directory::exact_filter(term)).await?;
    match exact.as_slice() {
        [only] => return Ok(Some(only.dn.clone())),
        [] => {}
        _ => return user_picker::pick(term, &exact, false),
    }

    let filter = directory::name_search_filter(term);
    let (candidates, truncated) = directory::search_users(ldap, base_dn, &filter).await?;
    match candidates.as_slice() {
        [] => Err(Error::NotFound {
            kind: "user",
            term: term.to_string(),
        }),
        [only] if !truncated => Ok(Some(only.dn.clone())),
        _ => user_picker::pick(term, &candidates, truncated),
    }
}
