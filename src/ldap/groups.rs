use ldap3::controls::PagedResults;
use ldap3::{Ldap, Scope, SearchEntry};
use serde::Serialize;

use crate::error::{AppError, Result};
use crate::ldap::{escape_filter_value, user_filter};

/// OID for the LDAP_MATCHING_RULE_IN_CHAIN extensible match, used to resolve
/// nested (transitive) group membership in one query family instead of
/// walking memberOf chains by hand.
const MATCHING_RULE_IN_CHAIN: &str = "1.2.840.113556.1.4.1941";
/// OID for the Simple Paged Results control (RFC 2696), needed because a
/// recursive membership query can exceed AD's default 1000-entry search limit.
const PAGED_RESULTS_OID: &str = "1.2.840.113556.1.4.319";
const PAGE_SIZE: i32 = 500;
/// AD's default MaxValRange: ranged attribute retrieval returns at most this
/// many values per response, requiring follow-up requests for the rest.
const RANGE_STEP: i64 = 1500;

#[derive(Debug, Clone, Serialize)]
pub struct GroupRecord {
    pub cn: String,
    pub dn: String,
    pub description: Option<String>,
}

/// Looks up the user and returns (user_dn, direct group DNs via memberOf),
/// transparently handling AD's ranged retrieval if the value count exceeds
/// the server's MaxValRange.
pub async fn fetch_direct_group_dns(
    ldap: &mut Ldap,
    base_dn: &str,
    identifier: &str,
) -> Result<(String, Vec<String>)> {
    let filter = user_filter(identifier);
    let (entries, _res) = ldap
        .search(
            base_dn,
            Scope::Subtree,
            &filter,
            vec!["distinguishedName", "memberOf"],
        )
        .await?
        .success()?;

    let entry = entries
        .into_iter()
        .next()
        .ok_or_else(|| AppError::UserNotFound(identifier.to_string()))?;
    let entry = SearchEntry::construct(entry);
    let user_dn = entry.dn;

    if let Some(values) = entry.attrs.get("memberOf") {
        return Ok((user_dn, values.clone()));
    }

    // Not present under the plain key: either the user is in zero groups, or
    // AD returned a ranged key like "memberOf;range=0-1499" instead.
    let ranged_key = entry
        .attrs
        .keys()
        .find(|k| k.starts_with("memberOf;range="))
        .cloned();

    let Some(first_key) = ranged_key else {
        return Ok((user_dn, Vec::new()));
    };

    let mut all_values = entry.attrs.get(&first_key).cloned().unwrap_or_default();
    let mut high = range_high(&first_key);

    while let Some(prev_high) = high {
        let next_low = prev_high + 1;
        let range_attr = format!("memberOf;range={next_low}-{}", next_low + RANGE_STEP - 1);
        let (entries, _res) = ldap
            .search(
                &user_dn,
                Scope::Base,
                "(objectClass=*)",
                vec![range_attr.as_str()],
            )
            .await?
            .success()?;

        let Some(entry) = entries.into_iter().next() else {
            break;
        };
        let entry = SearchEntry::construct(entry);
        let Some((key, values)) = entry.attrs.iter().find(|(k, _)| k.starts_with("memberOf"))
        else {
            break;
        };

        all_values.extend(values.clone());
        high = range_high(key);
    }

    Ok((user_dn, all_values))
}

/// Parses the upper bound out of a ranged attribute key, e.g.
/// "memberOf;range=1500-2999" -> Some(2999), "memberOf;range=3000-*" -> None
/// (None signals "this was the last page").
fn range_high(key: &str) -> Option<i64> {
    let range_part = key.split("range=").nth(1)?;
    let high = range_part.split('-').nth(1)?;
    high.parse().ok()
}

/// Resolves every group the user is a member of, directly or transitively,
/// by querying from the group side with the LDAP_MATCHING_RULE_IN_CHAIN
/// extensible match. Paged automatically since the result set can be large.
pub async fn fetch_recursive_groups(
    ldap: &mut Ldap,
    base_dn: &str,
    user_dn: &str,
) -> Result<Vec<GroupRecord>> {
    let escaped_dn = escape_filter_value(user_dn);
    let filter = format!("(member:{MATCHING_RULE_IN_CHAIN}:={escaped_dn})");
    let attrs = vec!["cn", "distinguishedName", "description"];

    let mut groups = Vec::new();
    let mut cookie: Vec<u8> = Vec::new();

    loop {
        let paged = PagedResults {
            size: PAGE_SIZE,
            cookie: cookie.clone(),
        };
        let (entries, res) = ldap
            .with_controls(paged)
            .search(base_dn, Scope::Subtree, &filter, attrs.clone())
            .await?
            .success()?;

        for e in entries {
            let e = SearchEntry::construct(e);
            groups.push(GroupRecord {
                cn: e
                    .attrs
                    .get("cn")
                    .and_then(|v| v.first())
                    .cloned()
                    .unwrap_or_default(),
                dn: e.dn,
                description: e.attrs.get("description").and_then(|v| v.first()).cloned(),
            });
        }

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

    Ok(groups)
}

/// Resolves direct-only group DNs into display-friendly records (cn,
/// description) via a single batched subtree search, rather than one round
/// trip per group — matters once a user is in 100+ groups.
pub async fn resolve_group_records(
    ldap: &mut Ldap,
    base_dn: &str,
    group_dns: &[String],
) -> Result<Vec<GroupRecord>> {
    if group_dns.is_empty() {
        return Ok(Vec::new());
    }

    let ors: String = group_dns
        .iter()
        .map(|dn| format!("(distinguishedName={})", escape_filter_value(dn)))
        .collect();
    let filter = format!("(|{ors})");

    let (entries, _res) = ldap
        .search(
            base_dn,
            Scope::Subtree,
            &filter,
            vec!["cn", "distinguishedName", "description"],
        )
        .await?
        .success()?;

    let mut by_dn: std::collections::HashMap<String, GroupRecord> = entries
        .into_iter()
        .map(SearchEntry::construct)
        .map(|e| {
            let cn = e
                .attrs
                .get("cn")
                .and_then(|v| v.first())
                .cloned()
                .unwrap_or_else(|| e.dn.clone());
            let description = e.attrs.get("description").and_then(|v| v.first()).cloned();
            (
                e.dn.clone(),
                GroupRecord {
                    cn,
                    dn: e.dn.clone(),
                    description,
                },
            )
        })
        .collect();

    Ok(group_dns
        .iter()
        .map(|dn| {
            by_dn.remove(dn).unwrap_or_else(|| GroupRecord {
                cn: dn.clone(),
                dn: dn.clone(),
                description: None,
            })
        })
        .collect())
}
