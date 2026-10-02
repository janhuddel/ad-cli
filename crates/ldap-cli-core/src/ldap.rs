use std::collections::HashMap;

use ldap3::controls::PagedResults;
use ldap3::{Ldap, LdapConnAsync, LdapConnSettings, Scope, SearchEntry};

use crate::config::{Config, Session};
use crate::error::{explain_bind_failure, Error, Result};

/// OID for the Simple Paged Results control (RFC 2696), needed because a
/// large result set can exceed the server's non-paged search limit (AD's
/// default is 1000 entries).
const PAGED_RESULTS_OID: &str = "1.2.840.113556.1.4.319";
const PAGE_SIZE: i32 = 500;

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
        .map_err(|e| Error::BindFailed(explain_bind_failure(&e.to_string())))?;
    Ok(())
}

/// Connects and, unless the session is anonymous, binds with the stored
/// credentials.
pub async fn open(session: &Session) -> Result<Ldap> {
    let mut ldap = connect(&session.config).await?;
    if let (Some(identity), Some(password)) = (&session.config.bind_identity, &session.password) {
        bind(&mut ldap, identity, password).await?;
    }
    Ok(ldap)
}

/// Base-scope read of `base_dn`: proves an anonymous connection actually
/// works, since without a bind nothing else touches the server.
pub async fn check_base_dn(ldap: &mut Ldap, base_dn: &str) -> Result<()> {
    ldap.search(base_dn, Scope::Base, "(objectClass=*)", vec!["1.1"])
        .await?
        .success()
        .map_err(|e| Error::Other(format!("cannot read base DN '{base_dn}': {e}")))?;
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

/// Subtree search that pages through the result set with the Simple Paged
/// Results control, since membership queries can exceed the server's
/// non-paged search limit.
pub async fn paged_search(
    ldap: &mut Ldap,
    base_dn: &str,
    filter: &str,
    attrs: &[&str],
) -> Result<Vec<SearchEntry>> {
    let mut all = Vec::new();
    let mut cookie: Vec<u8> = Vec::new();

    loop {
        let paged = PagedResults {
            size: PAGE_SIZE,
            cookie: cookie.clone(),
        };
        let (entries, res) = ldap
            .with_controls(paged)
            .search(base_dn, Scope::Subtree, filter, attrs.to_vec())
            .await?
            .success()?;

        all.extend(entries.into_iter().map(SearchEntry::construct));

        let next_cookie = res
            .ctrls
            .iter()
            .find(|c| c.1.ctype == PAGED_RESULTS_OID)
            .map(|c| c.1.parse::<PagedResults>().cookie);

        match next_cookie {
            Some(c) if !c.is_empty() => cookie = c,
            _ => break,
        }
    }

    Ok(all)
}

/// First value of a (possibly multi-valued) attribute.
pub fn first(attrs: &HashMap<String, Vec<String>>, key: &str) -> Option<String> {
    attrs.get(key).and_then(|v| v.first()).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_filter_value_escapes_rfc4515_specials() {
        assert_eq!(escape_filter_value("a*(b)\\c\0"), "a\\2a\\28b\\29\\5cc\\00");
        assert_eq!(escape_filter_value("Max Müller"), "Max Müller");
    }
}
