//! Everything that depends on the IdM's LDAP schema. The attribute lists
//! below are educated guesses for an eDirectory-based IdM and the first
//! place to adjust when lookups don't find what they should.

use std::collections::BTreeMap;

use ldap3::{Ldap, Scope, SearchEntry, SearchOptions, SearchResult};
use ldap_cli_core::ldap::escape_filter_value;
use ldap_cli_core::{Error, Result};
use serde::Serialize;

/// Attributes an identifier is matched against exactly. eDirectory names
/// users by `cn`; the others are common alternatives in IdM schemas.
/// Attributes the server doesn't know simply never match.
const ID_ATTRS: &[&str] = &["cn", "uid", "mail", "workforceID"];

/// Attributes the name search looks for substrings in.
const NAME_ATTRS: &[&str] = &["cn", "givenName", "sn", "fullName", "mail"];

/// Attributes needed to label a search hit.
const CANDIDATE_ATTRS: &[&str] = &["cn", "fullName", "givenName", "sn", "mail"];

/// Multi-valued attribute on the user holding the assigned rights.
pub const RIGHTS_ATTR: &str = "rightvalue";

/// Cap on name-search hits; beyond this the term is too vague to be useful.
pub const SEARCH_LIMIT: i32 = 50;

/// LDAP result code for sizeLimitExceeded: the server still returns the
/// entries up to the limit, so this is a partial success, not a failure.
const RC_SIZE_LIMIT_EXCEEDED: u32 = 4;

/// A user entry with all attributes the server returned, sorted by name.
#[derive(Debug, Clone, Serialize)]
pub struct UserEntry {
    pub dn: String,
    pub attributes: BTreeMap<String, Vec<String>>,
}

impl UserEntry {
    /// Values of `name`, matched case-insensitively since servers return
    /// attribute names in their schema spelling (e.g. `rightValue`).
    pub fn values(&self, name: &str) -> &[String] {
        self.attributes
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_slice())
            .unwrap_or_default()
    }

    pub fn first(&self, name: &str) -> Option<&str> {
        self.values(name).first().map(String::as_str)
    }

    /// The user's rights, sorted case-insensitively and deduplicated.
    pub fn rights(&self) -> Vec<String> {
        let mut rights = self.values(RIGHTS_ATTR).to_vec();
        rights.sort_by_cached_key(|r| r.to_lowercase());
        rights.dedup();
        rights
    }

    /// Best human-readable name: fullName, else "givenName sn", else cn.
    pub fn display_name(&self) -> Option<String> {
        display_name(
            self.first("fullName"),
            self.first("givenName"),
            self.first("sn"),
        )
        .or_else(|| self.first("cn").map(str::to_string))
    }
}

#[derive(Debug, Clone)]
pub struct UserCandidate {
    pub dn: String,
    pub cn: String,
    pub name: Option<String>,
    pub mail: Option<String>,
}

fn display_name(full: Option<&str>, given: Option<&str>, sn: Option<&str>) -> Option<String> {
    if let Some(full) = full.filter(|f| !f.is_empty()) {
        return Some(full.to_string());
    }
    let joined = [given, sn]
        .into_iter()
        .flatten()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    (!joined.is_empty()).then_some(joined)
}

/// Exact match of `identifier` against any of `ID_ATTRS`.
pub fn exact_filter(identifier: &str) -> String {
    let e = escape_filter_value(identifier);
    let alternatives: String = ID_ATTRS.iter().map(|a| format!("({a}={e})")).collect();
    format!("(|{alternatives})")
}

/// Name search: every whitespace-separated word of `term` must occur as a
/// substring in one of `NAME_ATTRS`, so "max mül" finds "Max Müller".
pub fn name_search_filter(term: &str) -> String {
    let words: String = term
        .split_whitespace()
        .map(|w| {
            let e = escape_filter_value(w);
            let alternatives: String = NAME_ATTRS.iter().map(|a| format!("({a}=*{e}*)")).collect();
            format!("(|{alternatives})")
        })
        .collect();
    format!("(&{words})")
}

/// Runs `filter` below `base_dn` and returns the hits as picker candidates,
/// sorted by name, plus whether the result was cut off at `SEARCH_LIMIT`.
pub async fn search_users(
    ldap: &mut Ldap,
    base_dn: &str,
    filter: &str,
) -> Result<(Vec<UserCandidate>, bool)> {
    let SearchResult(entries, res) = ldap
        .with_search_options(SearchOptions::new().sizelimit(SEARCH_LIMIT))
        .search(base_dn, Scope::Subtree, filter, CANDIDATE_ATTRS.to_vec())
        .await?;
    let truncated = res.rc == RC_SIZE_LIMIT_EXCEEDED;
    if !truncated {
        res.success()?;
    }

    let mut candidates: Vec<UserCandidate> = entries
        .into_iter()
        .map(|e| to_entry(SearchEntry::construct(e)))
        .map(|u| UserCandidate {
            cn: u.first("cn").unwrap_or(&u.dn).to_string(),
            name: display_name(u.first("fullName"), u.first("givenName"), u.first("sn")),
            mail: u.first("mail").map(str::to_string),
            dn: u.dn,
        })
        .collect();
    candidates.sort_by_cached_key(|c| c.name.as_deref().unwrap_or(&c.cn).to_lowercase());
    Ok((candidates, truncated))
}

/// Reads every user attribute of the entry at `dn`.
pub async fn read_user(ldap: &mut Ldap, dn: &str) -> Result<UserEntry> {
    let (entries, _res) = ldap
        .search(dn, Scope::Base, "(objectClass=*)", vec!["*"])
        .await?
        .success()?;
    let entry = entries.into_iter().next().ok_or_else(|| Error::NotFound {
        kind: "user",
        term: dn.to_string(),
    })?;
    Ok(to_entry(SearchEntry::construct(entry)))
}

fn to_entry(e: SearchEntry) -> UserEntry {
    UserEntry {
        dn: e.dn,
        attributes: e.attrs.into_iter().collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_filter_escapes_and_covers_id_attrs() {
        assert_eq!(
            exact_filter("a*b"),
            "(|(cn=a\\2ab)(uid=a\\2ab)(mail=a\\2ab)(workforceID=a\\2ab))"
        );
    }

    #[test]
    fn name_search_filter_requires_every_word() {
        assert_eq!(
            name_search_filter(" max  mül "),
            "(&(|(cn=*max*)(givenName=*max*)(sn=*max*)(fullName=*max*)(mail=*max*))\
             (|(cn=*mül*)(givenName=*mül*)(sn=*mül*)(fullName=*mül*)(mail=*mül*)))"
        );
    }

    #[test]
    fn rights_are_case_insensitive_sorted_and_deduplicated() {
        let user = UserEntry {
            dn: "cn=x".into(),
            attributes: BTreeMap::from([(
                "rightValue".to_string(),
                vec!["b".into(), "A".into(), "b".into()],
            )]),
        };
        assert_eq!(user.rights(), vec!["A", "b"]);
    }

    #[test]
    fn display_name_prefers_full_name_then_given_and_sn() {
        assert_eq!(
            display_name(Some("Max Müller"), Some("X"), None).as_deref(),
            Some("Max Müller")
        );
        assert_eq!(
            display_name(Some(""), Some("Max"), Some("Müller")).as_deref(),
            Some("Max Müller")
        );
        assert_eq!(display_name(None, None, None), None);
    }
}
