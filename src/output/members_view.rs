use comfy_table::{presets::UTF8_FULL, Table};

use crate::cli::MembersOutputFormat;
use crate::error::{AppError, Result};
use crate::ldap::groups::MemberRecord;
use crate::output::{fuzzy_picker, stdout_is_interactive};

/// Same split as `groups_view::display`: interactive fuzzy filter for a human
/// at a terminal, CSV/JSON/plain-table whenever stdout isn't a TTY or a
/// format is given explicitly.
pub fn display(
    members: &[MemberRecord],
    format: Option<MembersOutputFormat>,
    no_interactive: bool,
) -> Result<()> {
    let want_interactive = format.is_none() && !no_interactive && stdout_is_interactive();

    match format {
        Some(MembersOutputFormat::Csv) => write_csv(members),
        Some(MembersOutputFormat::Json) => write_json(members),
        Some(MembersOutputFormat::Table) => {
            render_table(members);
            Ok(())
        }
        None if want_interactive => interactive_filter(members),
        None => {
            render_table(members);
            Ok(())
        }
    }
}

fn enabled_text(enabled: Option<bool>) -> &'static str {
    match enabled {
        Some(true) => "yes",
        Some(false) => "no",
        None => "",
    }
}

fn render_table(members: &[MemberRecord]) {
    let mut table = Table::new();
    table.load_style(UTF8_FULL);
    table.set_header(vec![
        "Name",
        "sAMAccountName",
        "Type",
        "Enabled",
        "Distinguished Name",
    ]);
    for m in members {
        table.add_row(vec![
            m.name.clone(),
            m.sam_account_name.clone().unwrap_or_default(),
            m.kind.clone(),
            enabled_text(m.enabled).to_string(),
            m.dn.clone(),
        ]);
    }
    println!("{} member(s) total", members.len());
    println!("{table}");
}

fn write_csv(members: &[MemberRecord]) -> Result<()> {
    let mut wtr = csv::Writer::from_writer(std::io::stdout());
    wtr.write_record(["name", "sam_account_name", "type", "enabled", "dn"])
        .map_err(|e| AppError::Other(e.to_string()))?;
    for m in members {
        wtr.write_record([
            m.name.as_str(),
            m.sam_account_name.as_deref().unwrap_or(""),
            m.kind.as_str(),
            match m.enabled {
                Some(true) => "true",
                Some(false) => "false",
                None => "",
            },
            m.dn.as_str(),
        ])
        .map_err(|e| AppError::Other(e.to_string()))?;
    }
    wtr.flush().map_err(AppError::Io)?;
    Ok(())
}

fn write_json(members: &[MemberRecord]) -> Result<()> {
    let text = serde_json::to_string_pretty(members).map_err(|e| AppError::Other(e.to_string()))?;
    println!("{text}");
    Ok(())
}

fn item_label(m: &MemberRecord) -> String {
    let mut label = m.name.clone();
    if let Some(sam) = m.sam_account_name.as_deref().filter(|s| *s != m.name) {
        label.push_str(&format!("  ({sam})"));
    }
    label.push_str("  ·  ");
    label.push_str(&m.kind);
    if m.enabled == Some(false) {
        label.push_str("  [disabled]");
    }
    label
}

fn interactive_filter(members: &[MemberRecord]) -> Result<()> {
    if members.is_empty() {
        println!("This group has no members.");
        return Ok(());
    }

    let items: Vec<String> = members.iter().map(item_label).collect();
    let prompt = format!(
        "{} member(s) — type to filter, Enter for detail, Esc to quit",
        members.len()
    );

    loop {
        let selection = fuzzy_picker::pick(&prompt, &items)?;

        let Some(index) = selection else {
            break;
        };
        let m = &members[index];
        println!("\nName:           {}", m.name);
        println!(
            "sAMAccountName: {}",
            m.sam_account_name.as_deref().unwrap_or("(none)")
        );
        println!("Type:           {}", m.kind);
        if m.enabled.is_some() {
            println!("Enabled:        {}", enabled_text(m.enabled));
        }
        println!("DN:             {}\n", m.dn);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member(name: &str, sam: Option<&str>, kind: &str, enabled: Option<bool>) -> MemberRecord {
        MemberRecord {
            name: name.into(),
            sam_account_name: sam.map(Into::into),
            kind: kind.into(),
            enabled,
            dn: format!("CN={name},DC=example,DC=com"),
        }
    }

    #[test]
    fn item_label_shows_sam_type_and_disabled() {
        assert_eq!(
            item_label(&member("Max Müller", Some("mmueller"), "user", Some(true))),
            "Max Müller  (mmueller)  ·  user"
        );
        assert_eq!(
            item_label(&member("Alt User", Some("alt"), "user", Some(false))),
            "Alt User  (alt)  ·  user  [disabled]"
        );
        assert_eq!(
            item_label(&member("App-Admins", Some("App-Admins"), "group", None)),
            "App-Admins  ·  group"
        );
        assert_eq!(
            item_label(&member("Ext Kontakt", None, "contact", None)),
            "Ext Kontakt  ·  contact"
        );
    }
}
