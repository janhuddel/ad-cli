pub mod groups;
pub mod login;
pub mod logout;
pub mod members;
pub mod user;
pub mod whoami;

use ldap3::Ldap;

use crate::error::{AppError, Result};
use crate::ldap::groups::{find_group_dn, search_groups};
use crate::ldap::user::{search_users, user_exists};
use crate::output::{group_picker, user_picker};

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

/// Turns what the user typed into a group DN, analogous to
/// `resolve_identifier`: an exact sAMAccountName/CN/DN match is used as-is,
/// otherwise a name search (ANR) with a picker for several hits. `None`
/// means the pick was aborted.
pub(crate) async fn resolve_group(
    ldap: &mut Ldap,
    base_dn: &str,
    input: &str,
) -> Result<Option<String>> {
    if let Some(dn) = find_group_dn(ldap, base_dn, input).await? {
        return Ok(Some(dn));
    }

    let (candidates, truncated) = search_groups(ldap, base_dn, input).await?;
    match candidates.as_slice() {
        [] => Err(AppError::GroupNotFound(input.to_string())),
        [only] if !truncated => Ok(Some(only.dn.clone())),
        _ => group_picker::pick(input, &candidates, truncated),
    }
}
