pub mod groups;
pub mod user;

use ldap3::{Ldap, LdapConnAsync, LdapConnSettings};

use crate::config::Config;
use crate::error::{AppError, Result};

pub async fn connect(config: &Config) -> Result<Ldap> {
    let mut settings = LdapConnSettings::new();
    if config.insecure_skip_verify {
        settings = settings.set_no_tls_verify(true);
    }
    let (conn, ldap) = LdapConnAsync::with_settings(settings, &config.ldaps_url()).await?;
    ldap3::drive!(conn);
    Ok(ldap)
}

pub async fn bind(ldap: &mut Ldap, bind_identity: &str, password: &str) -> Result<()> {
    let result = ldap.simple_bind(bind_identity, password).await?;
    result
        .success()
        .map_err(|e| AppError::BindFailed(crate::error::explain_bind_failure(&e.to_string())))?;
    Ok(())
}

/// Escapes a value for safe interpolation into an LDAP filter, per RFC 4515.
/// `ldap3` does not do this automatically, so every user-supplied identifier
/// must be passed through this before being placed in a filter string.
pub fn escape_filter_value(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\5c"),
            '*' => out.push_str("\\2a"),
            '(' => out.push_str("\\28"),
            ')' => out.push_str("\\29"),
            '\0' => out.push_str("\\00"),
            _ => out.push(c),
        }
    }
    out
}

/// Filter used to locate a user by either sAMAccountName or userPrincipalName,
/// since the identifier the caller supplies could be in either form.
pub fn user_filter(identifier: &str) -> String {
    let escaped = escape_filter_value(identifier);
    format!("(|(sAMAccountName={escaped})(userPrincipalName={escaped}))")
}

/// Ambiguous Name Resolution filter: AD prefix-matches the term against
/// displayName, givenName, sn, sAMAccountName, mail etc. server-side, and
/// splits "first last" into a combined givenName/sn match. Restricted to
/// person/user objects so computers and contacts don't show up.
pub fn anr_filter(term: &str) -> String {
    let escaped = escape_filter_value(term);
    format!("(&(objectCategory=person)(objectClass=user)(anr={escaped}))")
}

/// Filter used to locate a group exactly by sAMAccountName, CN or DN.
pub fn group_filter(identifier: &str) -> String {
    let escaped = escape_filter_value(identifier);
    format!(
        "(&(objectCategory=group)(|(sAMAccountName={escaped})(cn={escaped})(distinguishedName={escaped})))"
    )
}

/// Group search filter: every whitespace-separated word of `term` must occur
/// somewhere (substring, not just prefix like ANR) in the group's CN,
/// sAMAccountName or description, so "sap admin" finds "GRP-SAP-Admins".
/// An empty term matches all groups.
pub fn group_search_filter(term: &str) -> String {
    let words: String = term
        .split_whitespace()
        .map(|w| {
            let e = escape_filter_value(w);
            format!("(|(cn=*{e}*)(sAMAccountName=*{e}*)(description=*{e}*))")
        })
        .collect();
    format!("(&(objectCategory=group){words})")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anr_filter_escapes_special_chars() {
        assert_eq!(
            anr_filter("m*(x)\\"),
            "(&(objectCategory=person)(objectClass=user)(anr=m\\2a\\28x\\29\\5c))"
        );
    }

    #[test]
    fn anr_filter_keeps_umlauts_and_spaces() {
        assert_eq!(
            anr_filter("Max Müller"),
            "(&(objectCategory=person)(objectClass=user)(anr=Max Müller))"
        );
    }

    #[test]
    fn group_filters_escape_special_chars() {
        assert_eq!(
            group_filter("g*(x)"),
            "(&(objectCategory=group)(|(sAMAccountName=g\\2a\\28x\\29)(cn=g\\2a\\28x\\29)(distinguishedName=g\\2a\\28x\\29)))"
        );
    }

    #[test]
    fn group_search_filter_requires_every_word_as_substring() {
        assert_eq!(
            group_search_filter(" sap  ad*min "),
            "(&(objectCategory=group)\
             (|(cn=*sap*)(sAMAccountName=*sap*)(description=*sap*))\
             (|(cn=*ad\\2amin*)(sAMAccountName=*ad\\2amin*)(description=*ad\\2amin*)))"
        );
        assert_eq!(group_search_filter(""), "(&(objectCategory=group))");
    }
}
