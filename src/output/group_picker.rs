use crate::error::{AppError, Result};
use crate::ldap::groups::GroupCandidate;
use crate::ldap::user::SEARCH_LIMIT;
use crate::output::{fuzzy_picker, stdout_is_interactive};

/// Lets the user choose among several group name-search hits and returns the
/// chosen group's DN. Same rules as `user_picker::pick`: fuzzy select on a
/// TTY (`None` if aborted), otherwise fail with the candidate list.
pub fn pick(term: &str, candidates: &[GroupCandidate], truncated: bool) -> Result<Option<String>> {
    if !stdout_is_interactive() {
        return Err(AppError::AmbiguousGroup {
            term: term.to_string(),
            candidates: candidate_list(candidates, truncated),
        });
    }

    let items: Vec<String> = candidates.iter().map(item_label).collect();
    let prompt = if truncated {
        format!("more than {SEARCH_LIMIT} groups match '{term}' (showing first {SEARCH_LIMIT}) — type to filter, Enter to select, Esc to quit")
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
fn candidate_list(candidates: &[GroupCandidate], truncated: bool) -> String {
    let mut lines: Vec<String> = candidates.iter().map(|c| format!("  {}", c.dn)).collect();
    if truncated {
        lines.push(format!(
            "  … more than {SEARCH_LIMIT} matches, please be more specific"
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
    fn candidate_list_shows_dns_and_notes_truncation() {
        assert_eq!(
            candidate_list(&[candidate("A", None)], true),
            "  CN=A,OU=Groups,DC=example,DC=com\n  … more than 50 matches, please be more specific"
        );
    }
}
