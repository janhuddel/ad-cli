use ldap_cli_core::{fuzzy_picker, stdout_is_interactive, Error, Result};

use crate::directory::{UserCandidate, SEARCH_LIMIT};

/// Lets the user choose among several search hits and returns the chosen
/// DN. On a TTY this is a fuzzy select (`None` if aborted with Esc); without
/// a TTY we never guess on a script's behalf and fail with the candidate
/// list instead.
pub fn pick(term: &str, candidates: &[UserCandidate], truncated: bool) -> Result<Option<String>> {
    if !stdout_is_interactive() {
        return Err(Error::Ambiguous {
            kinds: "users",
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

    let selection = fuzzy_picker::pick(&prompt, &items)?;
    Ok(selection.map(|i| candidates[i].dn.clone()))
}

fn item_label(c: &UserCandidate) -> String {
    let mut label = format!("{}  ({})", c.name.as_deref().unwrap_or(&c.cn), c.cn);
    if let Some(mail) = c.mail.as_deref().filter(|m| !m.is_empty()) {
        label.push_str("  ·  ");
        label.push_str(mail);
    }
    label
}

fn candidate_list(candidates: &[UserCandidate], truncated: bool) -> String {
    let width = candidates
        .iter()
        .map(|c| c.cn.chars().count())
        .max()
        .unwrap_or(0);
    let mut lines: Vec<String> = candidates
        .iter()
        .map(|c| {
            format!("  {:width$}  {}", c.cn, c.name.as_deref().unwrap_or(""))
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

    fn candidate(cn: &str, name: Option<&str>, mail: Option<&str>) -> UserCandidate {
        UserCandidate {
            dn: format!("cn={cn},ou=users,o=example"),
            cn: cn.into(),
            name: name.map(Into::into),
            mail: mail.map(Into::into),
        }
    }

    #[test]
    fn item_label_includes_mail_when_set() {
        assert_eq!(
            item_label(&candidate(
                "U123",
                Some("Max Müller"),
                Some("max@example.com")
            )),
            "Max Müller  (U123)  ·  max@example.com"
        );
        assert_eq!(item_label(&candidate("U9", None, Some(""))), "U9  (U9)");
    }

    #[test]
    fn candidate_list_aligns_ids_and_notes_truncation() {
        assert_eq!(
            candidate_list(
                &[
                    candidate("U1234", Some("Anna Müller"), None),
                    candidate("U1", None, None)
                ],
                true,
            ),
            "  U1234  Anna Müller\n  U1\n  … more than 50 matches, please be more specific"
        );
    }
}
