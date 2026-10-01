use std::collections::HashMap;

use chrono::{DateTime, NaiveDateTime, Utc};
use ldap3::{Ldap, Scope, SearchEntry, SearchOptions, SearchResult};
use serde::Serialize;

use crate::error::{AppError, Result};
use crate::ldap::{anr_filter, user_filter};

const USER_ATTRS: &[&str] = &[
    "displayName",
    "mail",
    "title",
    "department",
    "physicalDeliveryOfficeName",
    "telephoneNumber",
    "mobile",
    "userAccountControl",
    "lastLogon",
    "lastLogonTimestamp",
    "whenCreated",
    "accountExpires",
    "distinguishedName",
    "sAMAccountName",
    "userPrincipalName",
];

// userAccountControl bit flags (relevant subset).
const UF_ACCOUNTDISABLE: u32 = 0x0002;
const UF_LOCKOUT: u32 = 0x0010;
const UF_DONT_EXPIRE_PASSWORD: u32 = 0x10000;
const UF_PASSWORD_EXPIRED: u32 = 0x800000;

#[derive(Debug, Clone, Serialize)]
pub struct UserRecord {
    pub sam_account_name: Option<String>,
    pub user_principal_name: Option<String>,
    pub display_name: Option<String>,
    pub mail: Option<String>,
    pub title: Option<String>,
    pub department: Option<String>,
    pub physical_delivery_office_name: Option<String>,
    pub telephone_number: Option<String>,
    pub mobile: Option<String>,
    pub distinguished_name: String,
    pub enabled: bool,
    pub account_flags: Vec<String>,
    pub last_logon: Option<String>,
    pub last_logon_timestamp: Option<String>,
    pub when_created: Option<String>,
    pub account_expires: Option<String>,
}

pub async fn find_user(ldap: &mut Ldap, base_dn: &str, identifier: &str) -> Result<UserRecord> {
    let filter = user_filter(identifier);
    let (entries, _res) = ldap
        .search(base_dn, Scope::Subtree, &filter, USER_ATTRS)
        .await?
        .success()?;

    let entry = entries
        .into_iter()
        .next()
        .ok_or_else(|| AppError::UserNotFound(identifier.to_string()))?;
    let entry = SearchEntry::construct(entry);
    Ok(parse_user_record(entry.dn, entry.attrs))
}

/// Cap on name-search hits; beyond this the term is too vague to be useful.
pub const SEARCH_LIMIT: i32 = 50;

/// LDAP result code for sizeLimitExceeded: the server still returns the
/// entries up to the limit, so this is a partial success, not a failure.
const RC_SIZE_LIMIT_EXCEEDED: u32 = 4;

#[derive(Debug, Clone)]
pub struct UserCandidate {
    pub sam_account_name: String,
    pub display_name: Option<String>,
    pub department: Option<String>,
}

/// True if `identifier` matches a user exactly by sAMAccountName or UPN.
pub async fn user_exists(ldap: &mut Ldap, base_dn: &str, identifier: &str) -> Result<bool> {
    let filter = user_filter(identifier);
    let (entries, _res) = ldap
        .search(base_dn, Scope::Subtree, &filter, vec!["sAMAccountName"])
        .await?
        .success()?;
    Ok(!entries.is_empty())
}

/// Name search via AD's Ambiguous Name Resolution. Returns the candidates
/// sorted by display name, plus whether the result was cut off at
/// `SEARCH_LIMIT`.
pub async fn search_users(
    ldap: &mut Ldap,
    base_dn: &str,
    term: &str,
) -> Result<(Vec<UserCandidate>, bool)> {
    let filter = anr_filter(term);
    let SearchResult(entries, res) = ldap
        .with_search_options(SearchOptions::new().sizelimit(SEARCH_LIMIT))
        .search(
            base_dn,
            Scope::Subtree,
            &filter,
            vec!["sAMAccountName", "displayName", "department"],
        )
        .await?;
    let truncated = res.rc == RC_SIZE_LIMIT_EXCEEDED;
    if !truncated {
        res.success()?;
    }

    let mut candidates: Vec<UserCandidate> = entries
        .into_iter()
        .map(SearchEntry::construct)
        .filter_map(|e| {
            Some(UserCandidate {
                sam_account_name: first(&e.attrs, "sAMAccountName")?,
                display_name: first(&e.attrs, "displayName"),
                department: first(&e.attrs, "department"),
            })
        })
        .collect();
    candidates.sort_by_cached_key(|c| {
        c.display_name
            .as_deref()
            .unwrap_or(&c.sam_account_name)
            .to_lowercase()
    });
    Ok((candidates, truncated))
}

fn first(attrs: &HashMap<String, Vec<String>>, key: &str) -> Option<String> {
    attrs.get(key).and_then(|v| v.first()).cloned()
}

fn parse_user_record(dn: String, attrs: HashMap<String, Vec<String>>) -> UserRecord {
    let uac: u32 = first(&attrs, "userAccountControl")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    let mut flags = Vec::new();
    if uac & UF_LOCKOUT != 0 {
        flags.push("locked out".to_string());
    }
    if uac & UF_DONT_EXPIRE_PASSWORD != 0 {
        flags.push("password never expires".to_string());
    }
    if uac & UF_PASSWORD_EXPIRED != 0 {
        flags.push("password expired".to_string());
    }

    UserRecord {
        sam_account_name: first(&attrs, "sAMAccountName"),
        user_principal_name: first(&attrs, "userPrincipalName"),
        display_name: first(&attrs, "displayName"),
        mail: first(&attrs, "mail"),
        title: first(&attrs, "title"),
        department: first(&attrs, "department"),
        physical_delivery_office_name: first(&attrs, "physicalDeliveryOfficeName"),
        telephone_number: first(&attrs, "telephoneNumber"),
        mobile: first(&attrs, "mobile"),
        distinguished_name: dn,
        enabled: uac & UF_ACCOUNTDISABLE == 0,
        account_flags: flags,
        last_logon: first(&attrs, "lastLogon").map(|v| format_filetime(&v)),
        last_logon_timestamp: first(&attrs, "lastLogonTimestamp").map(|v| format_filetime(&v)),
        when_created: first(&attrs, "whenCreated").map(|v| format_generalized_time(&v)),
        account_expires: first(&attrs, "accountExpires").map(|v| format_account_expires(&v)),
    }
}

/// Windows FILETIME: 100ns ticks since 1601-01-01.
fn filetime_to_datetime(ticks: i64) -> Option<DateTime<Utc>> {
    const FILETIME_TO_UNIX_EPOCH_SECONDS: i64 = 11_644_473_600;
    let unix_seconds = ticks / 10_000_000 - FILETIME_TO_UNIX_EPOCH_SECONDS;
    let nanos = (ticks % 10_000_000) * 100;
    DateTime::from_timestamp(unix_seconds, nanos as u32)
}

/// lastLogon / lastLogonTimestamp: FILETIME, 0 means "never logged on".
fn format_filetime(raw: &str) -> String {
    let ticks: i64 = match raw.parse() {
        Ok(v) => v,
        Err(_) => return raw.to_string(),
    };
    if ticks == 0 {
        return "never".to_string();
    }
    filetime_to_datetime(ticks)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| raw.to_string())
}

/// accountExpires: FILETIME, but 0 or i64::MAX (0x7FFFFFFFFFFFFFFF) both mean
/// "never expires" rather than a real timestamp.
fn format_account_expires(raw: &str) -> String {
    let ticks: i64 = match raw.parse() {
        Ok(v) => v,
        Err(_) => return raw.to_string(),
    };
    if ticks == 0 || ticks == i64::MAX {
        return "never expires".to_string();
    }
    filetime_to_datetime(ticks)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| raw.to_string())
}

/// AD "generalized time" string, e.g. "20230101120000.0Z".
fn format_generalized_time(raw: &str) -> String {
    NaiveDateTime::parse_from_str(raw, "%Y%m%d%H%M%S%.fZ")
        .map(|dt| dt.and_utc().to_rfc3339())
        .unwrap_or_else(|_| raw.to_string())
}
