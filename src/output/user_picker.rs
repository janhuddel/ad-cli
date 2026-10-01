use dialoguer::FuzzySelect;

use crate::error::{AppError, Result};
use crate::ldap::user::{UserCandidate, SEARCH_LIMIT};
use crate::output::stdout_is_interactive;

/// Lets the user choose among several name-search hits. On a TTY this is a
/// fuzzy select (`None` if aborted with Esc); without a TTY we never guess
/// on a script's behalf and fail with the candidate list instead.
pub fn pick(term: &str, candidates: &[UserCandidate], truncated: bool) -> Result<Option<String>> {
    if !stdout_is_interactive() {
        return Err(AppError::AmbiguousUser {
            term: term.to_string(),
            candidates: candidate_list(candidates, truncated),
        });
    }

    let items: Vec<String> = candidates.iter().map(item_label).collect();
    let prompt = if truncated {
        format!("more than {SEARCH_LIMIT} users match '{term}' (showing first {SEARCH_LIMIT}) — type to filter, Enter to select, Esc to quit")
    } else {
        format!(
            "{} users match '{term}' — type to filter, Enter to select, Esc to quit",
            candidates.len()
        )
    };

    let selection = FuzzySelect::new()
        .with_prompt(&prompt)
        .items(&items)
        .default(0)
        .interact_opt()
        .map_err(|e| AppError::Other(e.to_string()))?;
    Ok(selection.map(|i| candidates[i].sam_account_name.clone()))
}

fn item_label(c: &UserCandidate) -> String {
    let mut label = format!(
        "{}  ({})",
        c.display_name.as_deref().unwrap_or(&c.sam_account_name),
        c.sam_account_name
    );
    if let Some(dept) = c.department.as_deref().filter(|d| !d.is_empty()) {
        label.push_str("  ·  ");
        label.push_str(dept);
    }
    label
}

fn candidate_list(candidates: &[UserCandidate], truncated: bool) -> String {
    let width = candidates
        .iter()
        .map(|c| c.sam_account_name.chars().count())
        .max()
        .unwrap_or(0);
    let mut lines: Vec<String> = candidates
        .iter()
        .map(|c| {
            format!(
                "  {:width$}  {}",
                c.sam_account_name,
                c.display_name.as_deref().unwrap_or("")
            )
            .trim_end()
            .to_string()
        })
        .collect();
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

    fn candidate(sam: &str, name: Option<&str>, dept: Option<&str>) -> UserCandidate {
        UserCandidate {
            sam_account_name: sam.into(),
            display_name: name.map(Into::into),
            department: dept.map(Into::into),
        }
    }

    #[test]
    fn item_label_includes_department_when_set() {
        assert_eq!(
            item_label(&candidate("mmueller", Some("Max Müller"), Some("IT"))),
            "Max Müller  (mmueller)  ·  IT"
        );
        assert_eq!(
            item_label(&candidate("svc_x", None, Some(""))),
            "svc_x  (svc_x)"
        );
    }

    #[test]
    fn candidate_list_aligns_names_and_notes_truncation() {
        let list = candidate_list(
            &[
                candidate("amueller", Some("Anna Müller"), None),
                candidate("mm", None, None),
            ],
            true,
        );
        assert_eq!(
            list,
            "  amueller  Anna Müller\n  mm\n  … more than 50 matches, please be more specific"
        );
    }
}
