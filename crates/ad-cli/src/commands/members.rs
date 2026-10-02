use std::path::PathBuf;

use ldap_cli_core::{config, Result};

use crate::cli::MembersArgs;
use crate::commands::resolve_group;
use crate::ldap::{self, groups};
use crate::output::members_view;
use crate::APP;

pub async fn run(args: MembersArgs, config_override: Option<&PathBuf>) -> Result<()> {
    let session = config::load_session(&APP, config_override)?;
    let mut conn = ldap::open(&session).await?;

    let input = args.group.join(" ");
    let Some(group_dn) = resolve_group(&mut conn, &session.config.base_dn, &input).await? else {
        let _ = conn.unbind().await;
        return Ok(());
    };
    let members = groups::fetch_members(
        &mut conn,
        &session.config.base_dn,
        &group_dn,
        args.recursive,
    )
    .await?;

    let _ = conn.unbind().await;

    members_view::display(&members, args.output, args.no_interactive)
}
