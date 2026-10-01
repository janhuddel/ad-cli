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
