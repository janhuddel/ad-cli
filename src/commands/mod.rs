pub mod groups;
pub mod login;
pub mod logout;
pub mod user;
pub mod whoami;

use ldap3::Ldap;

use crate::error::{AppError, Result};
use crate::ldap::user::{search_users, user_exists};
use crate::output::user_picker;

/// Turns what the user typed into an exact sAMAccountName/UPN. An exact
/// match is used as-is; otherwise falls back to a name search (ANR) and,
/// with several hits, lets the user pick one. `None` means the pick was
/// aborted.
pub(crate) async fn resolve_identifier(
    ldap: &mut Ldap,
    base_dn: &str,
    input: &str,
) -> Result<Option<String>> {
    if user_exists(ldap, base_dn, input).await? {
        return Ok(Some(input.to_string()));
    }

    let (candidates, truncated) = search_users(ldap, base_dn, input).await?;
    match candidates.as_slice() {
        [] => Err(AppError::UserNotFound(input.to_string())),
        [only] if !truncated => Ok(Some(only.sam_account_name.clone())),
        _ => user_picker::pick(input, &candidates, truncated),
    }
}
