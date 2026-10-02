use std::path::PathBuf;

use ldap_cli_core::{config, Result};

use crate::cli::GroupsArgs;
use crate::commands::resolve_identifier;
use crate::ldap::{self, groups};
use crate::output::groups_view;
use crate::APP;

pub async fn run(args: GroupsArgs, config_override: Option<&PathBuf>) -> Result<()> {
    let session = config::load_session(&APP, config_override)?;
    let mut conn = ldap::open(&session).await?;

    let Some(identifier) =
        resolve_identifier(&mut conn, &session.config.base_dn, &args.identifier).await?
    else {
        let _ = conn.unbind().await;
        return Ok(());
    };
    let (user_dn, direct_dns) =
        groups::fetch_direct_group_dns(&mut conn, &session.config.base_dn, &identifier).await?;

    let records = if args.recursive {
        groups::fetch_recursive_groups(&mut conn, &session.config.base_dn, &user_dn).await?
    } else {
        groups::resolve_group_records(&mut conn, &session.config.base_dn, &direct_dns).await?
    };

    let _ = conn.unbind().await;

    groups_view::display(&records, args.output, args.no_interactive)
}
