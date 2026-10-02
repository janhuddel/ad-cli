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
use crate::output::{group_picker, stdout_is_interactive, user_picker};

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

/// Turns what the user typed into a group DN: an exact sAMAccountName/CN/DN
/// match is used as-is, otherwise a substring search whose hits go to the
/// fuzzy picker (a single hit is used directly). Without input every group
/// is offered in the picker. `None` means the pick was aborted.
pub(crate) async fn resolve_group(
    ldap: &mut Ldap,
    base_dn: &str,
    input: &str,
) -> Result<Option<String>> {
    let term = input.trim();
    if term.is_empty() && !stdout_is_interactive() {
        return Err(AppError::Other(
            "No group given. Pass a group name, or run in a terminal to pick from all groups."
                .to_string(),
        ));
    }
    if !term.is_empty() {
        if let Some(dn) = find_group_dn(ldap, base_dn, term).await? {
            return Ok(Some(dn));
        }
    }

    let candidates = search_groups(ldap, base_dn, term).await?;
    match candidates.as_slice() {
        [] if term.is_empty() => Err(AppError::Other("No groups found.".to_string())),
        [] => Err(AppError::GroupNotFound(term.to_string())),
        [only] if !term.is_empty() => Ok(Some(only.dn.clone())),
        _ => group_picker::pick(term, &candidates),
    }
}
