use console::style;
use ldap_cli_core::{Error, Result};

use crate::directory::{UserEntry, RIGHTS_ATTR};

/// Longest attribute name that still sets the label column width; longer
/// names just push their own line's values to the right.
const MAX_LABEL_WIDTH: usize = 24;

/// Default view: name and DN, then every attribute the server returned
/// (sorted, one value per line), since the IdM schema varies. The rights
/// are only counted here — `idm rights` lists them.
pub fn render_compact(user: &UserEntry, stage: &str) {
    print!("{}", compact_card(user, stage));
}

fn compact_card(user: &UserEntry, stage: &str) -> String {
    let mut out = String::new();
    let name = user.display_name().unwrap_or_else(|| user.dn.clone());
    out.push_str(&style(name).bold().to_string());
    out.push_str(&style(format!("  ·  stage {stage}")).dim().to_string());
    out.push('\n');
    out.push_str(&style(&user.dn).dim().to_string());
    out.push_str("\n\n");

    let mut attrs: Vec<(&String, &Vec<String>)> = user
        .attributes
        .iter()
        .filter(|(_, v)| v.iter().any(|s| !s.is_empty()))
        .collect();
    attrs.sort_by_cached_key(|(k, _)| k.to_lowercase());
    let width = attrs
        .iter()
        .map(|(k, _)| k.chars().count())
        .filter(|&w| w <= MAX_LABEL_WIDTH)
        .max()
        .unwrap_or(0);

    for (key, values) in attrs {
        if key.eq_ignore_ascii_case(RIGHTS_ATTR) {
            let summary = format!("{} value(s) — run 'idm rights' to list them", values.len());
            push_line(&mut out, key, width, &summary);
            continue;
        }
        for (i, value) in values.iter().enumerate() {
            push_line(&mut out, if i == 0 { key } else { "" }, width, value);
        }
    }
    out
}

fn push_line(out: &mut String, label: &str, width: usize, value: &str) {
    let padded = format!("{label:width$}");
    out.push_str(&style(padded).dim().to_string());
    out.push_str("  ");
    out.push_str(value);
    out.push('\n');
}

pub fn render_json(user: &UserEntry) -> Result<()> {
    let text = serde_json::to_string_pretty(user).map_err(|e| Error::Other(e.to_string()))?;
    println!("{text}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    #[test]
    fn compact_card_lists_attributes_and_summarizes_rights() {
        console::set_colors_enabled(false);
        let user = UserEntry {
            dn: "cn=U1,ou=users,o=example".into(),
            attributes: BTreeMap::from([
                ("cn".to_string(), vec!["U1".to_string()]),
                ("fullName".to_string(), vec!["Max Müller".to_string()]),
                (
                    "mail".to_string(),
                    vec!["a@x".to_string(), "b@x".to_string()],
                ),
                ("empty".to_string(), vec![String::new()]),
                (
                    "rightValue".to_string(),
                    vec!["r1".to_string(), "r2".to_string()],
                ),
            ]),
        };
        assert_eq!(
            compact_card(&user, "prod"),
            "Max Müller  ·  stage prod\ncn=U1,ou=users,o=example\n\n\
             cn          U1\n\
             fullName    Max Müller\n\
             mail        a@x\n\
             \x20           b@x\n\
             rightValue  2 value(s) — run 'idm rights' to list them\n"
        );
    }
}
