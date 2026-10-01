use std::path::PathBuf;

use crate::cli::GroupsArgs;
use crate::config;
use crate::error::Result;
use crate::ldap::{self, groups};
use crate::output::groups_view;

pub async fn run(args: GroupsArgs, config_override: Option<&PathBuf>) -> Result<()> {
    let session = config::load_session(config_override)?;
    let mut conn = ldap::connect(&session.config).await?;
    ldap::bind(&mut conn, &session.config.bind_identity, &session.password).await?;

    let (user_dn, direct_dns) =
        groups::fetch_direct_group_dns(&mut conn, &session.config.base_dn, &args.identifier)
            .await?;

    let records = if args.recursive {
        groups::fetch_recursive_groups(&mut conn, &session.config.base_dn, &user_dn).await?
    } else {
        groups::resolve_group_records(&mut conn, &session.config.base_dn, &direct_dns).await?
    };

    let _ = conn.unbind().await;

    groups_view::display(&records, args.output, args.no_interactive)
}
