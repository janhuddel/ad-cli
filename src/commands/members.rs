use std::path::PathBuf;

use crate::cli::MembersArgs;
use crate::commands::resolve_group;
use crate::config;
use crate::error::Result;
use crate::ldap::{self, groups};
use crate::output::members_view;

pub async fn run(args: MembersArgs, config_override: Option<&PathBuf>) -> Result<()> {
    let session = config::load_session(config_override)?;
    let mut conn = ldap::connect(&session.config).await?;
    ldap::bind(&mut conn, &session.config.bind_identity, &session.password).await?;

    let Some(group_dn) = resolve_group(&mut conn, &session.config.base_dn, &args.group).await?
    else {
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
