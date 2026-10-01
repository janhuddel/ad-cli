use std::path::PathBuf;

use crate::cli::{UserArgs, UserOutputFormat};
use crate::config;
use crate::error::Result;
use crate::ldap::{self, user::find_user};
use crate::output::user_view;

pub async fn run(args: UserArgs, config_override: Option<&PathBuf>) -> Result<()> {
    let session = config::load_session(config_override)?;
    let mut conn = ldap::connect(&session.config).await?;
    ldap::bind(&mut conn, &session.config.bind_identity, &session.password).await?;

    let user = find_user(&mut conn, &session.config.base_dn, &args.identifier).await?;
    let _ = conn.unbind().await;

    match args.output {
        UserOutputFormat::Table => user_view::render_table(&user),
        UserOutputFormat::Json => user_view::render_json(&user)?,
    }
    Ok(())
}
