use crate::error::{AppError, Result};
use crate::ldap::groups::GroupCandidate;
use crate::output::{fuzzy_picker, stdout_is_interactive};

/// Shown at most in the non-interactive candidate list.
const MAX_LISTED: usize = 50;

/// Lets the user choose among several group search hits (or all groups, for
/// an empty `term`) and returns the chosen group's DN. Same rules as
/// `user_picker::pick`: fuzzy select on a TTY (`None` if aborted), otherwise
/// fail with the candidate list.
pub fn pick(term: &str, candidates: &[GroupCandidate]) -> Result<Option<String>> {
    if !stdout_is_interactive() {
        return Err(AppError::AmbiguousGroup {
            term: term.to_string(),
            candidates: candidate_list(candidates),
        });
    }

    let items: Vec<String> = candidates.iter().map(item_label).collect();
    let prompt = if term.is_empty() {
        format!(
            "{} groups — type to filter, Enter to select, Esc to quit",
            candidates.len()
        )
    } else {
        format!(
            "{} groups match '{term}' — type to filter, Enter to select, Esc to quit",
            candidates.len()
        )
    };

    let selection = fuzzy_picker::pick(&prompt, &items)?;
    Ok(selection.map(|i| candidates[i].dn.clone()))
}

fn item_label(c: &GroupCandidate) -> String {
    let mut label = c.cn.clone();
    if let Some(desc) = c.description.as_deref().filter(|d| !d.is_empty()) {
        label.push_str("  ·  ");
        label.push_str(desc);
    }
    label
}

/// Lists DNs rather than CNs: the same CN can exist in several OUs, and the
/// DN is something the caller can pass back verbatim.
fn candidate_list(candidates: &[GroupCandidate]) -> String {
    let mut lines: Vec<String> = candidates
        .iter()
        .take(MAX_LISTED)
        .map(|c| format!("  {}", c.dn))
        .collect();
    if candidates.len() > MAX_LISTED {
        lines.push(format!(
            "  … and {} more, please be more specific",
            candidates.len() - MAX_LISTED
        ));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(cn: &str, desc: Option<&str>) -> GroupCandidate {
        GroupCandidate {
            cn: cn.into(),
            dn: format!("CN={cn},OU=Groups,DC=example,DC=com"),
            description: desc.map(Into::into),
        }
    }

    #[test]
    fn item_label_includes_description_when_set() {
        assert_eq!(
            item_label(&candidate("App-Admins", Some("Admins der App"))),
            "App-Admins  ·  Admins der App"
        );
        assert_eq!(item_label(&candidate("App-Users", Some(""))), "App-Users");
    }

    #[test]
    fn candidate_list_shows_dns_and_caps_length() {
        assert_eq!(
            candidate_list(&[candidate("A", None)]),
            "  CN=A,OU=Groups,DC=example,DC=com"
        );
        let many: Vec<_> = (0..53).map(|i| candidate(&format!("G{i}"), None)).collect();
        let list = candidate_list(&many);
        assert_eq!(list.lines().count(), MAX_LISTED + 1);
        assert!(list.ends_with("  … and 3 more, please be more specific"));
    }
}
