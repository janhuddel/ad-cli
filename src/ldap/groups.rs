use ldap3::controls::PagedResults;
use ldap3::{Ldap, Scope, SearchEntry, SearchOptions, SearchResult};
use serde::Serialize;

use crate::error::{AppError, Result};
use crate::ldap::user::{first, RC_SIZE_LIMIT_EXCEEDED, SEARCH_LIMIT};
use crate::ldap::{escape_filter_value, group_anr_filter, group_filter, user_filter};

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
    let entries = paged_search(
        ldap,
        base_dn,
        &filter,
        &["cn", "distinguishedName", "description"],
    )
    .await?;

    Ok(entries
        .into_iter()
        .map(|e| GroupRecord {
            cn: first(&e.attrs, "cn").unwrap_or_default(),
            description: first(&e.attrs, "description"),
            dn: e.dn,
        })
        .collect())
}

/// Subtree search that pages through the result set with the Simple Paged
/// Results control, since membership queries can exceed AD's default
/// 1000-entry non-paged search limit.
async fn paged_search(
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

#[derive(Debug, Clone)]
pub struct GroupCandidate {
    pub cn: String,
    pub dn: String,
    pub description: Option<String>,
}

/// Returns the DN of the group matching `identifier` exactly by
/// sAMAccountName, CN or DN, if there is exactly one such group. Several
/// exact hits (same CN in different OUs) return `None` so the caller falls
/// back to the name search and its picker.
pub async fn find_group_dn(
    ldap: &mut Ldap,
    base_dn: &str,
    identifier: &str,
) -> Result<Option<String>> {
    let filter = group_filter(identifier);
    let (entries, _res) = ldap
        .search(base_dn, Scope::Subtree, &filter, vec!["distinguishedName"])
        .await?
        .success()?;
    match <[_; 1]>::try_from(entries) {
        Ok([entry]) => Ok(Some(SearchEntry::construct(entry).dn)),
        Err(_) => Ok(None),
    }
}

/// Group name search via AD's Ambiguous Name Resolution. Returns the
/// candidates sorted by CN, plus whether the result was cut off at
/// `SEARCH_LIMIT`.
pub async fn search_groups(
    ldap: &mut Ldap,
    base_dn: &str,
    term: &str,
) -> Result<(Vec<GroupCandidate>, bool)> {
    let filter = group_anr_filter(term);
    let SearchResult(entries, res) = ldap
        .with_search_options(SearchOptions::new().sizelimit(SEARCH_LIMIT))
        .search(
            base_dn,
            Scope::Subtree,
            &filter,
            vec!["cn", "distinguishedName", "description"],
        )
        .await?;
    let truncated = res.rc == RC_SIZE_LIMIT_EXCEEDED;
    if !truncated {
        res.success()?;
    }

    let mut candidates: Vec<GroupCandidate> = entries
        .into_iter()
        .map(SearchEntry::construct)
        .map(|e| GroupCandidate {
            cn: first(&e.attrs, "cn").unwrap_or_else(|| e.dn.clone()),
            description: first(&e.attrs, "description"),
            dn: e.dn,
        })
        .collect();
    candidates.sort_by_cached_key(|c| c.cn.to_lowercase());
    Ok((candidates, truncated))
}

// userAccountControl: account disabled.
const UF_ACCOUNTDISABLE: u32 = 0x0002;

#[derive(Debug, Clone, Serialize)]
pub struct MemberRecord {
    /// displayName, falling back to cn.
    pub name: String,
    pub sam_account_name: Option<String>,
    /// "user", "group", "computer", "contact" or "other".
    #[serde(rename = "type")]
    pub kind: String,
    /// Only set for objects carrying userAccountControl (users, computers).
    pub enabled: Option<bool>,
    pub dn: String,
}

/// Lists the members of a group, optionally including transitive members of
/// nested groups. Queried from the member side (`memberOf`) rather than by
/// reading the group's `member` attribute: that returns the display
/// attributes in the same round trip and sidesteps `member`'s ranged
/// retrieval for groups with more than 1500 members.
///
/// Membership via a user's *primary group* (`primaryGroupID`, e.g. "Domain
/// Users") is stored in neither `member` nor `memberOf` and is therefore not
/// listed.
pub async fn fetch_members(
    ldap: &mut Ldap,
    base_dn: &str,
    group_dn: &str,
    recursive: bool,
) -> Result<Vec<MemberRecord>> {
    let escaped_dn = escape_filter_value(group_dn);
    let filter = if recursive {
        format!("(memberOf:{MATCHING_RULE_IN_CHAIN}:={escaped_dn})")
    } else {
        format!("(memberOf={escaped_dn})")
    };
    let entries = paged_search(
        ldap,
        base_dn,
        &filter,
        &[
            "sAMAccountName",
            "displayName",
            "cn",
            "objectClass",
            "userAccountControl",
            "distinguishedName",
        ],
    )
    .await?;

    let mut members: Vec<MemberRecord> = entries
        .into_iter()
        .map(|e| {
            let classes = e.attrs.get("objectClass").cloned().unwrap_or_default();
            MemberRecord {
                name: first(&e.attrs, "displayName")
                    .or_else(|| first(&e.attrs, "cn"))
                    .unwrap_or_else(|| e.dn.clone()),
                sam_account_name: first(&e.attrs, "sAMAccountName"),
                kind: member_kind(&classes).to_string(),
                enabled: first(&e.attrs, "userAccountControl")
                    .and_then(|v| v.parse::<u32>().ok())
                    .map(|uac| uac & UF_ACCOUNTDISABLE == 0),
                dn: e.dn,
            }
        })
        .collect();
    members.sort_by_cached_key(|m| m.name.to_lowercase());
    Ok(members)
}

/// Maps an entry's objectClass chain to a coarse type. Order matters:
/// computer objects also carry the "user" class.
fn member_kind(object_classes: &[String]) -> &'static str {
    let has = |c: &str| object_classes.iter().any(|v| v.eq_ignore_ascii_case(c));
    if has("computer") {
        "computer"
    } else if has("group") {
        "group"
    } else if has("contact") {
        "contact"
    } else if has("user") {
        "user"
    } else {
        "other"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn classes(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn member_kind_prefers_computer_over_user() {
        assert_eq!(
            member_kind(&classes(&[
                "top",
                "person",
                "organizationalPerson",
                "user",
                "computer"
            ])),
            "computer"
        );
        assert_eq!(
            member_kind(&classes(&["top", "person", "organizationalPerson", "user"])),
            "user"
        );
        assert_eq!(member_kind(&classes(&["top", "group"])), "group");
        assert_eq!(
            member_kind(&classes(&[
                "top",
                "person",
                "organizationalPerson",
                "contact"
            ])),
            "contact"
        );
        assert_eq!(
            member_kind(&classes(&["top", "foreignSecurityPrincipal"])),
            "other"
        );
    }
}
